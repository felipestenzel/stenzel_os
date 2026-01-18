//! LUKS-like Disk Encryption
//!
//! Linux Unified Key Setup (LUKS) compatible disk encryption.
//! Features: AES-XTS encryption, PBKDF2 key derivation, multiple key slots.

#![allow(dead_code)]

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use alloc::vec;
use core::sync::atomic::{AtomicBool, Ordering};

use crate::crypto::aes::{aes_encrypt_block, aes_decrypt_block, expand_key_256};
use crate::crypto::sha256::{sha256, hmac_sha256};
use crate::sync::IrqSafeMutex;

/// LUKS magic signature
pub const LUKS_MAGIC: &[u8; 6] = b"LUKS\xba\xbe";

/// LUKS version 2
pub const LUKS_VERSION: u16 = 2;

/// Number of key slots
pub const LUKS_NUM_SLOTS: usize = 8;

/// Key slot states
pub const LUKS_KEY_DISABLED: u32 = 0x0000DEAD;
pub const LUKS_KEY_ENABLED: u32 = 0x00AC71F3;

/// Cipher modes
pub const CIPHER_AES_XTS_PLAIN64: &str = "aes-xts-plain64";
pub const CIPHER_AES_CBC_ESSIV: &str = "aes-cbc-essiv:sha256";

/// PBKDF2 iterations (configurable, higher = more secure but slower)
pub const DEFAULT_PBKDF2_ITERATIONS: u32 = 100_000;

/// Key sizes
pub const KEY_SIZE_128: usize = 16;
pub const KEY_SIZE_256: usize = 32;
pub const KEY_SIZE_512: usize = 64; // For XTS (two keys)

/// Sector size (standard 512 bytes)
pub const SECTOR_SIZE: usize = 512;

/// LUKS header size (4096 bytes typical)
pub const LUKS_HEADER_SIZE: usize = 4096;

// ============================================================================
// LUKS Header Structures
// ============================================================================

/// LUKS key slot
#[derive(Clone)]
pub struct LuksKeySlot {
    /// Slot state (enabled/disabled)
    pub active: u32,
    /// PBKDF2 iterations
    pub iterations: u32,
    /// Salt for key derivation (32 bytes)
    pub salt: [u8; 32],
    /// Start sector of key material
    pub key_material_offset: u32,
    /// Number of anti-forensic stripes
    pub stripes: u32,
}

impl LuksKeySlot {
    pub fn new() -> Self {
        Self {
            active: LUKS_KEY_DISABLED,
            iterations: DEFAULT_PBKDF2_ITERATIONS,
            salt: [0u8; 32],
            key_material_offset: 0,
            stripes: 4000,
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.active == LUKS_KEY_ENABLED
    }
}

impl Default for LuksKeySlot {
    fn default() -> Self {
        Self::new()
    }
}

/// LUKS header
#[derive(Clone)]
pub struct LuksHeader {
    /// Magic signature
    pub magic: [u8; 6],
    /// LUKS version
    pub version: u16,
    /// Cipher name
    pub cipher_name: String,
    /// Cipher mode
    pub cipher_mode: String,
    /// Hash specification
    pub hash_spec: String,
    /// Payload offset (in sectors)
    pub payload_offset: u32,
    /// Key size (in bytes)
    pub key_bytes: u32,
    /// Master key checksum
    pub mk_digest: [u8; 20],
    /// Master key salt
    pub mk_digest_salt: [u8; 32],
    /// PBKDF2 iterations for MK digest
    pub mk_digest_iter: u32,
    /// UUID
    pub uuid: String,
    /// Key slots
    pub key_slots: [LuksKeySlot; LUKS_NUM_SLOTS],
}

impl LuksHeader {
    pub fn new() -> Self {
        Self {
            magic: *LUKS_MAGIC,
            version: LUKS_VERSION,
            cipher_name: String::from("aes"),
            cipher_mode: String::from("xts-plain64"),
            hash_spec: String::from("sha256"),
            payload_offset: 4096, // 2MB default offset (4096 * 512 = 2MB)
            key_bytes: 64, // 512-bit for XTS
            mk_digest: [0u8; 20],
            mk_digest_salt: [0u8; 32],
            mk_digest_iter: DEFAULT_PBKDF2_ITERATIONS,
            uuid: String::new(),
            key_slots: core::array::from_fn(|_| LuksKeySlot::new()),
        }
    }

    /// Serialize header to bytes
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut buf = vec![0u8; LUKS_HEADER_SIZE];

        // Magic (0-5)
        buf[0..6].copy_from_slice(&self.magic);
        // Version (6-7)
        buf[6..8].copy_from_slice(&self.version.to_be_bytes());
        // Cipher name (8-39) - 32 bytes
        let cipher = self.cipher_name.as_bytes();
        buf[8..8 + cipher.len().min(32)].copy_from_slice(&cipher[..cipher.len().min(32)]);
        // Cipher mode (40-71) - 32 bytes
        let mode = self.cipher_mode.as_bytes();
        buf[40..40 + mode.len().min(32)].copy_from_slice(&mode[..mode.len().min(32)]);
        // Hash spec (72-103) - 32 bytes
        let hash = self.hash_spec.as_bytes();
        buf[72..72 + hash.len().min(32)].copy_from_slice(&hash[..hash.len().min(32)]);
        // Payload offset (104-107)
        buf[104..108].copy_from_slice(&self.payload_offset.to_be_bytes());
        // Key bytes (108-111)
        buf[112..116].copy_from_slice(&self.key_bytes.to_be_bytes());
        // MK digest (116-135) - 20 bytes
        buf[116..136].copy_from_slice(&self.mk_digest);
        // MK digest salt (136-167) - 32 bytes
        buf[136..168].copy_from_slice(&self.mk_digest_salt);
        // MK digest iterations (168-171)
        buf[168..172].copy_from_slice(&self.mk_digest_iter.to_be_bytes());
        // UUID (172-211) - 40 bytes
        let uuid = self.uuid.as_bytes();
        buf[172..172 + uuid.len().min(40)].copy_from_slice(&uuid[..uuid.len().min(40)]);

        // Key slots start at offset 208
        let mut offset = 208;
        for slot in &self.key_slots {
            buf[offset..offset + 4].copy_from_slice(&slot.active.to_be_bytes());
            buf[offset + 4..offset + 8].copy_from_slice(&slot.iterations.to_be_bytes());
            buf[offset + 8..offset + 40].copy_from_slice(&slot.salt);
            buf[offset + 40..offset + 44].copy_from_slice(&slot.key_material_offset.to_be_bytes());
            buf[offset + 44..offset + 48].copy_from_slice(&slot.stripes.to_be_bytes());
            offset += 48;
        }

        buf
    }

    /// Parse header from bytes
    pub fn from_bytes(data: &[u8]) -> Option<Self> {
        if data.len() < LUKS_HEADER_SIZE {
            return None;
        }

        // Check magic
        if &data[0..6] != LUKS_MAGIC {
            return None;
        }

        let version = u16::from_be_bytes([data[6], data[7]]);

        // Read cipher name
        let cipher_name = core::str::from_utf8(&data[8..40])
            .ok()?
            .trim_end_matches('\0')
            .to_string();

        // Read cipher mode
        let cipher_mode = core::str::from_utf8(&data[40..72])
            .ok()?
            .trim_end_matches('\0')
            .to_string();

        // Read hash spec
        let hash_spec = core::str::from_utf8(&data[72..104])
            .ok()?
            .trim_end_matches('\0')
            .to_string();

        let payload_offset = u32::from_be_bytes([data[104], data[105], data[106], data[107]]);
        let key_bytes = u32::from_be_bytes([data[112], data[113], data[114], data[115]]);

        let mut mk_digest = [0u8; 20];
        mk_digest.copy_from_slice(&data[116..136]);

        let mut mk_digest_salt = [0u8; 32];
        mk_digest_salt.copy_from_slice(&data[136..168]);

        let mk_digest_iter = u32::from_be_bytes([data[168], data[169], data[170], data[171]]);

        let uuid = core::str::from_utf8(&data[172..212])
            .ok()?
            .trim_end_matches('\0')
            .to_string();

        // Read key slots
        let mut key_slots: [LuksKeySlot; LUKS_NUM_SLOTS] = core::array::from_fn(|_| LuksKeySlot::new());
        let mut offset = 208;
        for slot in &mut key_slots {
            slot.active = u32::from_be_bytes([
                data[offset], data[offset + 1], data[offset + 2], data[offset + 3]
            ]);
            slot.iterations = u32::from_be_bytes([
                data[offset + 4], data[offset + 5], data[offset + 6], data[offset + 7]
            ]);
            slot.salt.copy_from_slice(&data[offset + 8..offset + 40]);
            slot.key_material_offset = u32::from_be_bytes([
                data[offset + 40], data[offset + 41], data[offset + 42], data[offset + 43]
            ]);
            slot.stripes = u32::from_be_bytes([
                data[offset + 44], data[offset + 45], data[offset + 46], data[offset + 47]
            ]);
            offset += 48;
        }

        Some(Self {
            magic: *LUKS_MAGIC,
            version,
            cipher_name,
            cipher_mode,
            hash_spec,
            payload_offset,
            key_bytes,
            mk_digest,
            mk_digest_salt,
            mk_digest_iter,
            uuid,
            key_slots,
        })
    }
}

impl Default for LuksHeader {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// PBKDF2 Key Derivation
// ============================================================================

/// PBKDF2-HMAC-SHA256 key derivation
pub fn pbkdf2_sha256(password: &[u8], salt: &[u8], iterations: u32, dk_len: usize) -> Vec<u8> {
    let mut dk = Vec::with_capacity(dk_len);
    let hash_len = 32; // SHA-256 output
    let blocks = (dk_len + hash_len - 1) / hash_len;

    for i in 1..=blocks as u32 {
        // First iteration: U_1 = HMAC(password, salt || INT(i))
        let mut block_salt = salt.to_vec();
        block_salt.extend_from_slice(&i.to_be_bytes());
        let mut u = hmac_sha256(password, &block_salt);
        let mut result = u;

        // Remaining iterations: U_j = HMAC(password, U_{j-1})
        for _ in 1..iterations {
            u = hmac_sha256(password, &u);
            // XOR into result
            for (r, v) in result.iter_mut().zip(u.iter()) {
                *r ^= v;
            }
        }

        dk.extend_from_slice(&result[..hash_len.min(dk_len - dk.len())]);
    }

    dk
}

// ============================================================================
// AES-XTS Implementation
// ============================================================================

/// Galois field multiplication by 2 in GF(2^128)
fn gf_mul2(tweak: &mut [u8; 16]) {
    let mut carry = 0u8;
    for i in 0..16 {
        let new_carry = (tweak[i] >> 7) & 1;
        tweak[i] = (tweak[i] << 1) | carry;
        carry = new_carry;
    }
    // XOR with 0x87 if there was a carry (reduction polynomial)
    if carry != 0 {
        tweak[0] ^= 0x87;
    }
}

/// AES-XTS encrypt a single 16-byte block
fn aes_xts_encrypt_block(
    block: &[u8; 16],
    key1: &[[u8; 16]; 15], // Expanded AES-256 key 1
    tweak: &[u8; 16],
) -> [u8; 16] {
    // XOR with tweak
    let mut data = [0u8; 16];
    for i in 0..16 {
        data[i] = block[i] ^ tweak[i];
    }

    // Encrypt
    let encrypted = aes_encrypt_block(&data, key1);

    // XOR with tweak again
    let mut result = [0u8; 16];
    for i in 0..16 {
        result[i] = encrypted[i] ^ tweak[i];
    }

    result
}

/// AES-XTS decrypt a single 16-byte block
fn aes_xts_decrypt_block(
    block: &[u8; 16],
    key1: &[[u8; 16]; 15], // Expanded AES-256 key 1
    tweak: &[u8; 16],
) -> [u8; 16] {
    // XOR with tweak
    let mut data = [0u8; 16];
    for i in 0..16 {
        data[i] = block[i] ^ tweak[i];
    }

    // Decrypt
    let decrypted = aes_decrypt_block(&data, key1);

    // XOR with tweak again
    let mut result = [0u8; 16];
    for i in 0..16 {
        result[i] = decrypted[i] ^ tweak[i];
    }

    result
}

/// Encrypt a sector using AES-XTS
pub fn aes_xts_encrypt_sector(
    plaintext: &[u8],
    key: &[u8], // 64 bytes: key1 (32) + key2 (32)
    sector_num: u64,
) -> Vec<u8> {
    if key.len() != 64 || plaintext.len() % 16 != 0 {
        return Vec::new();
    }

    let key1 = expand_key_256(&key[0..32].try_into().unwrap());
    let key2 = expand_key_256(&key[32..64].try_into().unwrap());

    // Generate initial tweak from sector number
    let mut tweak_block = [0u8; 16];
    tweak_block[0..8].copy_from_slice(&sector_num.to_le_bytes());
    let mut tweak = aes_encrypt_block(&tweak_block, &key2);

    let mut ciphertext = Vec::with_capacity(plaintext.len());
    let blocks = plaintext.len() / 16;

    for i in 0..blocks {
        let block: [u8; 16] = plaintext[i * 16..(i + 1) * 16].try_into().unwrap();
        let encrypted = aes_xts_encrypt_block(&block, &key1, &tweak);
        ciphertext.extend_from_slice(&encrypted);
        gf_mul2(&mut tweak);
    }

    ciphertext
}

/// Decrypt a sector using AES-XTS
pub fn aes_xts_decrypt_sector(
    ciphertext: &[u8],
    key: &[u8], // 64 bytes: key1 (32) + key2 (32)
    sector_num: u64,
) -> Vec<u8> {
    if key.len() != 64 || ciphertext.len() % 16 != 0 {
        return Vec::new();
    }

    let key1 = expand_key_256(&key[0..32].try_into().unwrap());
    let key2 = expand_key_256(&key[32..64].try_into().unwrap());

    // Generate initial tweak from sector number
    let mut tweak_block = [0u8; 16];
    tweak_block[0..8].copy_from_slice(&sector_num.to_le_bytes());
    let mut tweak = aes_encrypt_block(&tweak_block, &key2);

    let mut plaintext = Vec::with_capacity(ciphertext.len());
    let blocks = ciphertext.len() / 16;

    for i in 0..blocks {
        let block: [u8; 16] = ciphertext[i * 16..(i + 1) * 16].try_into().unwrap();
        let decrypted = aes_xts_decrypt_block(&block, &key1, &tweak);
        plaintext.extend_from_slice(&decrypted);
        gf_mul2(&mut tweak);
    }

    plaintext
}

// ============================================================================
// LUKS Volume
// ============================================================================

/// Error types for LUKS operations
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LuksError {
    InvalidHeader,
    InvalidPassword,
    NoEmptySlot,
    SlotDisabled,
    IoError,
    KeyDerivationFailed,
    EncryptionFailed,
    DecryptionFailed,
    AlreadyOpen,
    NotOpen,
}

/// State of a LUKS volume
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LuksState {
    Closed,
    Open,
}

/// LUKS encrypted volume
pub struct LuksVolume {
    /// Volume header
    pub header: LuksHeader,
    /// Master key (only set when unlocked)
    master_key: Option<Vec<u8>>,
    /// Current state
    state: LuksState,
    /// Device name
    device_name: String,
}

impl LuksVolume {
    /// Create a new LUKS volume with given master key
    pub fn new(device_name: &str, master_key: &[u8]) -> Self {
        let mut header = LuksHeader::new();

        // Generate UUID
        header.uuid = generate_uuid();

        // Hash master key for digest
        let mk_digest = pbkdf2_sha256(
            master_key,
            &header.mk_digest_salt,
            header.mk_digest_iter,
            20,
        );
        header.mk_digest.copy_from_slice(&mk_digest[0..20]);

        Self {
            header,
            master_key: Some(master_key.to_vec()),
            state: LuksState::Closed,
            device_name: String::from(device_name),
        }
    }

    /// Parse LUKS volume from header data
    pub fn from_header(device_name: &str, header_data: &[u8]) -> Option<Self> {
        let header = LuksHeader::from_bytes(header_data)?;
        Some(Self {
            header,
            master_key: None,
            state: LuksState::Closed,
            device_name: String::from(device_name),
        })
    }

    /// Add a key slot with the given passphrase
    pub fn add_key_slot(&mut self, passphrase: &[u8]) -> Result<usize, LuksError> {
        // Find an empty slot
        let slot_idx = self.header.key_slots
            .iter()
            .position(|s| !s.is_enabled())
            .ok_or(LuksError::NoEmptySlot)?;

        let master_key = self.master_key.as_ref().ok_or(LuksError::NotOpen)?;

        // Generate random salt
        let mut salt = [0u8; 32];
        generate_random_bytes(&mut salt);

        // Derive key from passphrase
        let derived_key = pbkdf2_sha256(
            passphrase,
            &salt,
            self.header.key_slots[slot_idx].iterations,
            master_key.len(),
        );

        // Encrypt master key with derived key
        // In real LUKS, this uses anti-forensic splitting
        // For simplicity, we just XOR here
        let _encrypted_mk: Vec<u8> = master_key
            .iter()
            .zip(derived_key.iter().cycle())
            .map(|(m, d)| m ^ d)
            .collect();

        // Update slot
        self.header.key_slots[slot_idx].active = LUKS_KEY_ENABLED;
        self.header.key_slots[slot_idx].salt = salt;
        self.header.key_slots[slot_idx].key_material_offset =
            (LUKS_HEADER_SIZE / SECTOR_SIZE) as u32 + (slot_idx as u32 * 128);

        Ok(slot_idx)
    }

    /// Remove a key slot
    pub fn remove_key_slot(&mut self, slot_idx: usize) -> Result<(), LuksError> {
        if slot_idx >= LUKS_NUM_SLOTS {
            return Err(LuksError::SlotDisabled);
        }

        // Don't allow removing the last key slot
        let enabled_count = self.header.key_slots.iter().filter(|s| s.is_enabled()).count();
        if enabled_count <= 1 {
            return Err(LuksError::SlotDisabled);
        }

        self.header.key_slots[slot_idx].active = LUKS_KEY_DISABLED;
        self.header.key_slots[slot_idx].salt = [0u8; 32];
        Ok(())
    }

    /// Unlock the volume with a passphrase
    pub fn unlock(&mut self, passphrase: &[u8]) -> Result<(), LuksError> {
        if self.state == LuksState::Open {
            return Err(LuksError::AlreadyOpen);
        }

        // Try each enabled slot
        for slot in &self.header.key_slots {
            if !slot.is_enabled() {
                continue;
            }

            // Derive key from passphrase
            let derived_key = pbkdf2_sha256(
                passphrase,
                &slot.salt,
                slot.iterations,
                self.header.key_bytes as usize,
            );

            // In real LUKS, we would read encrypted key material from disk
            // and decrypt it. For this implementation, we verify against
            // the master key digest.
            let mk_digest = pbkdf2_sha256(
                &derived_key,
                &self.header.mk_digest_salt,
                self.header.mk_digest_iter,
                20,
            );

            if mk_digest[0..20] == self.header.mk_digest {
                self.master_key = Some(derived_key);
                self.state = LuksState::Open;
                return Ok(());
            }
        }

        Err(LuksError::InvalidPassword)
    }

    /// Lock the volume (clear master key from memory)
    pub fn lock(&mut self) {
        if let Some(ref mut mk) = self.master_key {
            // Securely clear the key
            for byte in mk.iter_mut() {
                *byte = 0;
            }
        }
        self.master_key = None;
        self.state = LuksState::Closed;
    }

    /// Check if volume is unlocked
    pub fn is_open(&self) -> bool {
        self.state == LuksState::Open
    }

    /// Encrypt a sector
    pub fn encrypt_sector(&self, plaintext: &[u8], sector_num: u64) -> Result<Vec<u8>, LuksError> {
        let master_key = self.master_key.as_ref().ok_or(LuksError::NotOpen)?;

        let ciphertext = aes_xts_encrypt_sector(plaintext, master_key, sector_num);
        if ciphertext.is_empty() {
            return Err(LuksError::EncryptionFailed);
        }
        Ok(ciphertext)
    }

    /// Decrypt a sector
    pub fn decrypt_sector(&self, ciphertext: &[u8], sector_num: u64) -> Result<Vec<u8>, LuksError> {
        let master_key = self.master_key.as_ref().ok_or(LuksError::NotOpen)?;

        let plaintext = aes_xts_decrypt_sector(ciphertext, master_key, sector_num);
        if plaintext.is_empty() {
            return Err(LuksError::DecryptionFailed);
        }
        Ok(plaintext)
    }

    /// Get the device name
    pub fn device_name(&self) -> &str {
        &self.device_name
    }

    /// Get the UUID
    pub fn uuid(&self) -> &str {
        &self.header.uuid
    }

    /// Get payload offset in sectors
    pub fn payload_offset(&self) -> u64 {
        self.header.payload_offset as u64
    }
}

// ============================================================================
// LUKS Volume Manager
// ============================================================================

/// Global manager for encrypted volumes
pub struct LuksManager {
    /// Open volumes
    volumes: IrqSafeMutex<Vec<LuksVolume>>,
}

impl LuksManager {
    pub fn new() -> Self {
        Self {
            volumes: IrqSafeMutex::new(Vec::new()),
        }
    }

    /// Format a device with LUKS encryption
    pub fn format(
        &self,
        device_name: &str,
        passphrase: &[u8],
    ) -> Result<LuksHeader, LuksError> {
        // Generate random master key (64 bytes for AES-XTS-256)
        let mut master_key = [0u8; 64];
        generate_random_bytes(&mut master_key);

        let mut volume = LuksVolume::new(device_name, &master_key);
        volume.add_key_slot(passphrase)?;

        let header = volume.header.clone();

        // In real implementation, write header to device here
        // write_header_to_device(device_name, &header.to_bytes())?;

        Ok(header)
    }

    /// Open an encrypted volume
    pub fn open(
        &self,
        device_name: &str,
        passphrase: &[u8],
        header_data: &[u8],
    ) -> Result<(), LuksError> {
        let mut volume = LuksVolume::from_header(device_name, header_data)
            .ok_or(LuksError::InvalidHeader)?;

        volume.unlock(passphrase)?;

        self.volumes.lock().push(volume);
        Ok(())
    }

    /// Close an encrypted volume
    pub fn close(&self, device_name: &str) -> Result<(), LuksError> {
        let mut volumes = self.volumes.lock();

        let idx = volumes.iter().position(|v| v.device_name() == device_name)
            .ok_or(LuksError::NotOpen)?;

        volumes[idx].lock();
        volumes.remove(idx);
        Ok(())
    }

    /// Get a volume by name
    pub fn get_volume(&self, device_name: &str) -> Option<&LuksVolume> {
        // Note: This is a simplified implementation
        // In real use, we'd need better lifetime management
        None
    }

    /// List open volumes
    pub fn list_open(&self) -> Vec<String> {
        self.volumes.lock().iter().map(|v| v.device_name().to_string()).collect()
    }

    /// Check if a device is LUKS formatted
    pub fn is_luks(header_data: &[u8]) -> bool {
        if header_data.len() < 6 {
            return false;
        }
        &header_data[0..6] == LUKS_MAGIC
    }

    /// Encrypt data for a volume
    pub fn encrypt_sector(
        &self,
        device_name: &str,
        plaintext: &[u8],
        sector_num: u64,
    ) -> Result<Vec<u8>, LuksError> {
        let volumes = self.volumes.lock();
        let volume = volumes.iter()
            .find(|v| v.device_name() == device_name)
            .ok_or(LuksError::NotOpen)?;
        volume.encrypt_sector(plaintext, sector_num)
    }

    /// Decrypt data for a volume
    pub fn decrypt_sector(
        &self,
        device_name: &str,
        ciphertext: &[u8],
        sector_num: u64,
    ) -> Result<Vec<u8>, LuksError> {
        let volumes = self.volumes.lock();
        let volume = volumes.iter()
            .find(|v| v.device_name() == device_name)
            .ok_or(LuksError::NotOpen)?;
        volume.decrypt_sector(ciphertext, sector_num)
    }
}

impl Default for LuksManager {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Global Instance
// ============================================================================

use spin::Once;

static LUKS_MANAGER: Once<LuksManager> = Once::new();

/// Initialize LUKS subsystem
pub fn init() {
    LUKS_MANAGER.call_once(LuksManager::new);
    crate::kprintln!("luks: Disk encryption subsystem initialized");
}

/// Get the global LUKS manager
pub fn manager() -> &'static LuksManager {
    LUKS_MANAGER.get().expect("LUKS not initialized")
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Generate random bytes (using system RNG)
fn generate_random_bytes(buf: &mut [u8]) {
    // Use the kernel's random number generator
    for byte in buf.iter_mut() {
        *byte = crate::crypto::random::get_random_u8();
    }
}

/// Generate a UUID string
fn generate_uuid() -> String {
    let mut bytes = [0u8; 16];
    generate_random_bytes(&mut bytes);

    // Set version (4) and variant (RFC 4122)
    bytes[6] = (bytes[6] & 0x0F) | 0x40;
    bytes[8] = (bytes[8] & 0x3F) | 0x80;

    // Format as UUID string
    alloc::format!(
        "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        bytes[0], bytes[1], bytes[2], bytes[3],
        bytes[4], bytes[5],
        bytes[6], bytes[7],
        bytes[8], bytes[9],
        bytes[10], bytes[11], bytes[12], bytes[13], bytes[14], bytes[15]
    )
}

// ============================================================================
// dm-crypt Style Block Device Integration
// ============================================================================

use crate::storage::BlockDevice;

/// Encrypted block device wrapper
pub struct EncryptedBlockDevice {
    /// Underlying block device
    inner: alloc::sync::Arc<dyn BlockDevice>,
    /// LUKS volume (must be unlocked)
    device_name: String,
    /// Payload offset in sectors
    payload_offset: u64,
    /// Is active
    active: AtomicBool,
}

impl EncryptedBlockDevice {
    pub fn new(
        inner: alloc::sync::Arc<dyn BlockDevice>,
        device_name: &str,
        payload_offset: u64,
    ) -> Self {
        Self {
            inner,
            device_name: String::from(device_name),
            payload_offset,
            active: AtomicBool::new(true),
        }
    }

    pub fn is_active(&self) -> bool {
        self.active.load(Ordering::Acquire)
    }

    pub fn deactivate(&self) {
        self.active.store(false, Ordering::Release);
    }
}

// ============================================================================
// Syscall Interface
// ============================================================================

use crate::util::KError;

/// Format a device with LUKS
pub fn sys_luks_format(device: &str, passphrase: &[u8]) -> Result<(), KError> {
    manager().format(device, passphrase)
        .map_err(|_| KError::IO)?;
    Ok(())
}

/// Open an encrypted volume
pub fn sys_luks_open(device: &str, passphrase: &[u8], header: &[u8]) -> Result<(), KError> {
    manager().open(device, passphrase, header)
        .map_err(|e| match e {
            LuksError::InvalidPassword => KError::PermissionDenied,
            LuksError::InvalidHeader => KError::Invalid,
            _ => KError::IO,
        })
}

/// Close an encrypted volume
pub fn sys_luks_close(device: &str) -> Result<(), KError> {
    manager().close(device)
        .map_err(|_| KError::NotFound)
}

/// Check if data is LUKS header
pub fn sys_is_luks(header: &[u8]) -> bool {
    LuksManager::is_luks(header)
}
