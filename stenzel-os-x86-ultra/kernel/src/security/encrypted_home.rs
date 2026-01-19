//! Encrypted Home Directory
//!
//! Per-user encrypted home directories similar to ecryptfs/fscrypt.
//! Supports automatic unlock on login.

#![allow(dead_code)]

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use alloc::vec;
use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use crate::sync::IrqSafeMutex;
use crate::util::kprintln;

/// Encrypted home state
static ENCRYPTED_HOME: IrqSafeMutex<Option<EncryptedHomeManager>> = IrqSafeMutex::new(None);

/// Statistics
static STATS: EncryptedHomeStats = EncryptedHomeStats::new();

/// Encryption method
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EncryptionMethod {
    /// fscrypt (Linux native)
    Fscrypt,
    /// eCryptfs (stacked filesystem)
    Ecryptfs,
    /// LUKS container
    Luks,
    /// dm-crypt
    DmCrypt,
    /// Native (Stenzel OS implementation)
    Native,
}

impl EncryptionMethod {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Fscrypt => "fscrypt",
            Self::Ecryptfs => "ecryptfs",
            Self::Luks => "luks",
            Self::DmCrypt => "dm-crypt",
            Self::Native => "native",
        }
    }

    pub fn is_block_based(&self) -> bool {
        matches!(self, Self::Luks | Self::DmCrypt)
    }

    pub fn is_file_based(&self) -> bool {
        matches!(self, Self::Fscrypt | Self::Ecryptfs | Self::Native)
    }
}

/// Cipher algorithm
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CipherAlgorithm {
    Aes256Xts,
    Aes256Gcm,
    Aes128Xts,
    ChaCha20Poly1305,
}

impl CipherAlgorithm {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Aes256Xts => "aes-xts-plain64",
            Self::Aes256Gcm => "aes-gcm",
            Self::Aes128Xts => "aes-xts-128",
            Self::ChaCha20Poly1305 => "chacha20-poly1305",
        }
    }

    pub fn key_size(&self) -> usize {
        match self {
            Self::Aes256Xts => 64, // 2 x 256-bit keys
            Self::Aes256Gcm => 32,
            Self::Aes128Xts => 32, // 2 x 128-bit keys
            Self::ChaCha20Poly1305 => 32,
        }
    }
}

/// Key derivation function
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyDerivation {
    Pbkdf2,
    Argon2id,
    Scrypt,
}

impl KeyDerivation {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pbkdf2 => "pbkdf2",
            Self::Argon2id => "argon2id",
            Self::Scrypt => "scrypt",
        }
    }
}

/// Home directory state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HomeState {
    /// Not encrypted
    Unencrypted,
    /// Encrypted but locked
    Locked,
    /// Encrypted and unlocked
    Unlocked,
    /// Being encrypted (migration)
    Encrypting,
    /// Being decrypted (migration)
    Decrypting,
    /// Error state
    Error,
}

impl HomeState {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Unencrypted => "unencrypted",
            Self::Locked => "locked",
            Self::Unlocked => "unlocked",
            Self::Encrypting => "encrypting",
            Self::Decrypting => "decrypting",
            Self::Error => "error",
        }
    }
}

/// Encrypted home configuration
#[derive(Debug, Clone)]
pub struct EncryptedHomeConfig {
    /// User ID
    pub uid: u32,
    /// Username
    pub username: String,
    /// Home directory path (unencrypted: /home/user, encrypted: /home/.ecryptfs/user)
    pub home_path: String,
    /// Mount point (where decrypted data appears)
    pub mount_point: String,
    /// Encryption method
    pub method: EncryptionMethod,
    /// Cipher
    pub cipher: CipherAlgorithm,
    /// Key derivation
    pub kdf: KeyDerivation,
    /// KDF iterations/memory (method specific)
    pub kdf_params: KdfParams,
    /// Auto-unlock on login
    pub auto_unlock: bool,
    /// Require password (can't use just biometrics)
    pub require_password: bool,
    /// Key escrow enabled (recovery)
    pub key_escrow: bool,
    /// Creation timestamp
    pub created_at: u64,
}

/// KDF parameters
#[derive(Debug, Clone)]
pub struct KdfParams {
    /// Iterations (PBKDF2)
    pub iterations: u32,
    /// Memory cost in KB (Argon2/Scrypt)
    pub memory_cost: u32,
    /// Time cost (Argon2)
    pub time_cost: u32,
    /// Parallelism (Argon2)
    pub parallelism: u32,
}

impl Default for KdfParams {
    fn default() -> Self {
        Self {
            iterations: 100000,
            memory_cost: 65536, // 64 MB
            time_cost: 3,
            parallelism: 4,
        }
    }
}

/// Wrapped key (encrypted master key)
#[derive(Debug, Clone)]
pub struct WrappedKey {
    /// Key type
    pub key_type: WrappedKeyType,
    /// Salt
    pub salt: [u8; 32],
    /// Encrypted key material
    pub encrypted_key: Vec<u8>,
    /// Nonce/IV
    pub nonce: [u8; 12],
    /// Authentication tag
    pub tag: [u8; 16],
}

/// Wrapped key type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WrappedKeyType {
    /// Password-derived
    Password,
    /// TPM sealed
    Tpm,
    /// Recovery key
    Recovery,
    /// Escrow key
    Escrow,
}

/// User home entry
#[derive(Debug)]
pub struct UserHome {
    /// Configuration
    pub config: EncryptedHomeConfig,
    /// Current state
    pub state: HomeState,
    /// Wrapped keys
    pub wrapped_keys: Vec<WrappedKey>,
    /// Master key (only when unlocked, zeroed otherwise)
    master_key: Option<[u8; 64]>,
    /// Last unlock time
    pub last_unlock: Option<u64>,
    /// Mount count
    pub mount_count: u64,
    /// Error message (if in error state)
    pub error: Option<String>,
}

impl UserHome {
    fn new(config: EncryptedHomeConfig) -> Self {
        Self {
            config,
            state: HomeState::Unencrypted,
            wrapped_keys: Vec::new(),
            master_key: None,
            last_unlock: None,
            mount_count: 0,
            error: None,
        }
    }

    /// Clear master key from memory
    fn clear_key(&mut self) {
        if let Some(ref mut key) = self.master_key {
            // Overwrite with zeros
            for byte in key.iter_mut() {
                *byte = 0;
            }
        }
        self.master_key = None;
    }
}

impl Drop for UserHome {
    fn drop(&mut self) {
        self.clear_key();
    }
}

/// Encrypted home error
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EncryptedHomeError {
    NotFound,
    AlreadyExists,
    AlreadyUnlocked,
    AlreadyLocked,
    WrongPassword,
    KeyDerivationFailed,
    EncryptionFailed,
    DecryptionFailed,
    MountFailed,
    UnmountFailed,
    MigrationFailed,
    TpmError,
    NoRecoveryKey,
    InvalidState,
    IoError,
    PermissionDenied,
}

impl EncryptedHomeError {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::NotFound => "User not found",
            Self::AlreadyExists => "Already encrypted",
            Self::AlreadyUnlocked => "Already unlocked",
            Self::AlreadyLocked => "Already locked",
            Self::WrongPassword => "Wrong password",
            Self::KeyDerivationFailed => "Key derivation failed",
            Self::EncryptionFailed => "Encryption failed",
            Self::DecryptionFailed => "Decryption failed",
            Self::MountFailed => "Mount failed",
            Self::UnmountFailed => "Unmount failed",
            Self::MigrationFailed => "Migration failed",
            Self::TpmError => "TPM error",
            Self::NoRecoveryKey => "No recovery key",
            Self::InvalidState => "Invalid state",
            Self::IoError => "I/O error",
            Self::PermissionDenied => "Permission denied",
        }
    }
}

pub type EncryptedHomeResult<T> = Result<T, EncryptedHomeError>;

/// Statistics
pub struct EncryptedHomeStats {
    homes_encrypted: AtomicU64,
    unlocks: AtomicU64,
    locks: AtomicU64,
    failed_unlocks: AtomicU64,
    migrations_started: AtomicU64,
    migrations_completed: AtomicU64,
}

impl EncryptedHomeStats {
    const fn new() -> Self {
        Self {
            homes_encrypted: AtomicU64::new(0),
            unlocks: AtomicU64::new(0),
            locks: AtomicU64::new(0),
            failed_unlocks: AtomicU64::new(0),
            migrations_started: AtomicU64::new(0),
            migrations_completed: AtomicU64::new(0),
        }
    }
}

/// Encrypted home manager
pub struct EncryptedHomeManager {
    /// User homes
    homes: Vec<UserHome>,
    /// Default encryption method
    default_method: EncryptionMethod,
    /// Default cipher
    default_cipher: CipherAlgorithm,
    /// Default KDF
    default_kdf: KeyDerivation,
    /// Generate recovery keys by default
    default_recovery: bool,
    /// TPM available
    tpm_available: bool,
}

impl EncryptedHomeManager {
    /// Create new manager
    pub fn new() -> Self {
        Self {
            homes: Vec::new(),
            default_method: EncryptionMethod::Native,
            default_cipher: CipherAlgorithm::Aes256Xts,
            default_kdf: KeyDerivation::Argon2id,
            default_recovery: true,
            tpm_available: false,
        }
    }

    /// Initialize
    pub fn init(&mut self) {
        kprintln!("encrypted-home: Initializing...");

        // Check TPM availability
        self.tpm_available = self.detect_tpm();

        // Load existing configurations
        self.load_configs();

        kprintln!("encrypted-home: {} encrypted homes configured", self.homes.len());
    }

    fn detect_tpm(&self) -> bool {
        // Would check TPM presence
        true
    }

    fn load_configs(&mut self) {
        // Would load from /etc/encrypted-home or similar
    }

    fn save_configs(&self) {
        // Would save to persistent storage
    }

    /// Set up encrypted home for user
    pub fn setup(
        &mut self,
        uid: u32,
        username: &str,
        password: &str,
        options: SetupOptions,
    ) -> EncryptedHomeResult<String> {
        kprintln!("encrypted-home: Setting up encrypted home for {}", username);

        // Check if already exists
        if self.homes.iter().any(|h| h.config.uid == uid) {
            return Err(EncryptedHomeError::AlreadyExists);
        }

        // Generate master key
        let master_key = self.generate_master_key();

        // Create configuration
        let config = EncryptedHomeConfig {
            uid,
            username: username.to_string(),
            home_path: alloc::format!("/home/.encrypted/{}", username),
            mount_point: alloc::format!("/home/{}", username),
            method: options.method.unwrap_or(self.default_method),
            cipher: options.cipher.unwrap_or(self.default_cipher),
            kdf: options.kdf.unwrap_or(self.default_kdf),
            kdf_params: options.kdf_params.unwrap_or_default(),
            auto_unlock: options.auto_unlock,
            require_password: options.require_password,
            key_escrow: options.key_escrow,
            created_at: crate::time::uptime_ms(),
        };

        // Create wrapped key from password
        let wrapped_password = self.wrap_key_with_password(
            &master_key,
            password,
            &config.kdf,
            &config.kdf_params,
        )?;

        // Create recovery key if requested
        let recovery_phrase = if self.default_recovery || options.create_recovery {
            let (wrapped_recovery, phrase) = self.create_recovery_key(&master_key)?;
            let mut home = UserHome::new(config.clone());
            home.wrapped_keys.push(wrapped_password);
            home.wrapped_keys.push(wrapped_recovery);
            home.state = HomeState::Locked;
            home.master_key = Some(master_key);

            self.homes.push(home);
            Some(phrase)
        } else {
            let mut home = UserHome::new(config.clone());
            home.wrapped_keys.push(wrapped_password);
            home.state = HomeState::Locked;
            home.master_key = Some(master_key);

            self.homes.push(home);
            None
        };

        // Create TPM wrapped key if available and requested
        if self.tpm_available && options.use_tpm {
            // Extract master key first to avoid borrow conflict
            let master_key_copy = self.homes.iter()
                .find(|h| h.config.uid == uid)
                .and_then(|h| h.master_key);

            if let Some(key) = master_key_copy {
                if let Ok(wrapped_tpm) = self.wrap_key_with_tpm(&key) {
                    if let Some(home) = self.homes.iter_mut().find(|h| h.config.uid == uid) {
                        home.wrapped_keys.push(wrapped_tpm);
                    }
                }
            }
        }

        // Create encrypted directory structure
        self.create_encrypted_structure(&config)?;

        self.save_configs();
        STATS.homes_encrypted.fetch_add(1, Ordering::Relaxed);

        kprintln!("encrypted-home: Setup complete for {}", username);

        Ok(recovery_phrase.unwrap_or_default())
    }

    /// Generate master key
    fn generate_master_key(&self) -> [u8; 64] {
        // Would use cryptographic RNG
        let mut key = [0u8; 64];
        // Placeholder - fill with "random" data
        for (i, byte) in key.iter_mut().enumerate() {
            *byte = (i * 17 + 42) as u8;
        }
        key
    }

    /// Wrap master key with password
    fn wrap_key_with_password(
        &self,
        master_key: &[u8; 64],
        password: &str,
        kdf: &KeyDerivation,
        params: &KdfParams,
    ) -> EncryptedHomeResult<WrappedKey> {
        // Generate salt
        let mut salt = [0u8; 32];
        for (i, byte) in salt.iter_mut().enumerate() {
            *byte = (i * 7 + 13) as u8;
        }

        // Derive key from password (placeholder)
        let derived = self.derive_key(password, &salt, kdf, params)?;

        // Encrypt master key with derived key
        let (encrypted, nonce, tag) = self.encrypt_key(master_key, &derived)?;

        Ok(WrappedKey {
            key_type: WrappedKeyType::Password,
            salt,
            encrypted_key: encrypted,
            nonce,
            tag,
        })
    }

    /// Derive key from password
    fn derive_key(
        &self,
        password: &str,
        salt: &[u8; 32],
        _kdf: &KeyDerivation,
        _params: &KdfParams,
    ) -> EncryptedHomeResult<[u8; 32]> {
        // Placeholder - simple derivation
        let mut key = [0u8; 32];
        for (i, byte) in password.bytes().enumerate() {
            key[i % 32] ^= byte ^ salt[i % 32];
        }
        Ok(key)
    }

    /// Encrypt key with derived key
    fn encrypt_key(
        &self,
        plaintext: &[u8; 64],
        _key: &[u8; 32],
    ) -> EncryptedHomeResult<(Vec<u8>, [u8; 12], [u8; 16])> {
        // Placeholder - would use AES-GCM
        let nonce = [0u8; 12];
        let tag = [0u8; 16];
        let encrypted = plaintext.to_vec();
        Ok((encrypted, nonce, tag))
    }

    /// Create recovery key
    fn create_recovery_key(
        &self,
        master_key: &[u8; 64],
    ) -> EncryptedHomeResult<(WrappedKey, String)> {
        // Generate recovery phrase (BIP39-style)
        let words = RECOVERY_WORDS;
        let mut phrase_words = Vec::new();

        for i in 0..24 {
            let idx = (master_key[i % 64] as usize * 17 + i) % words.len();
            phrase_words.push(words[idx]);
        }

        let phrase = phrase_words.join("-");

        // Derive key from phrase
        let mut salt = [0u8; 32];
        for (i, byte) in salt.iter_mut().enumerate() {
            *byte = (i * 11 + 23) as u8;
        }

        let derived = self.derive_key(&phrase, &salt, &KeyDerivation::Pbkdf2, &KdfParams::default())?;
        let (encrypted, nonce, tag) = self.encrypt_key(master_key, &derived)?;

        Ok((WrappedKey {
            key_type: WrappedKeyType::Recovery,
            salt,
            encrypted_key: encrypted,
            nonce,
            tag,
        }, phrase))
    }

    /// Wrap key with TPM
    fn wrap_key_with_tpm(&self, master_key: &[u8; 64]) -> EncryptedHomeResult<WrappedKey> {
        // Would seal to TPM
        let salt = [0u8; 32];
        let nonce = [0u8; 12];
        let tag = [0u8; 16];
        let encrypted = master_key.to_vec();

        Ok(WrappedKey {
            key_type: WrappedKeyType::Tpm,
            salt,
            encrypted_key: encrypted,
            nonce,
            tag,
        })
    }

    /// Create encrypted directory structure
    fn create_encrypted_structure(&self, config: &EncryptedHomeConfig) -> EncryptedHomeResult<()> {
        kprintln!("encrypted-home: Creating structure at {}", config.home_path);
        // Would create directories, set permissions, etc.
        Ok(())
    }

    /// Unlock encrypted home
    pub fn unlock(&mut self, uid: u32, password: &str) -> EncryptedHomeResult<()> {
        kprintln!("encrypted-home: Unlocking home for uid {}", uid);

        let home_idx = self.homes.iter().position(|h| h.config.uid == uid)
            .ok_or(EncryptedHomeError::NotFound)?;

        let home = &self.homes[home_idx];

        if home.state == HomeState::Unlocked {
            return Err(EncryptedHomeError::AlreadyUnlocked);
        }

        if home.state != HomeState::Locked {
            return Err(EncryptedHomeError::InvalidState);
        }

        // Find password wrapped key
        let wrapped = home.wrapped_keys.iter()
            .find(|k| k.key_type == WrappedKeyType::Password)
            .ok_or(EncryptedHomeError::NotFound)?;

        // Derive key from password
        let derived = self.derive_key(
            password,
            &wrapped.salt,
            &home.config.kdf,
            &home.config.kdf_params,
        )?;

        // Decrypt master key
        let master_key = self.decrypt_key(&wrapped.encrypted_key, &derived, &wrapped.nonce, &wrapped.tag)?;

        // Mount encrypted filesystem
        let config = home.config.clone();
        self.mount_encrypted(&config, &master_key)?;

        // Update state
        let home = &mut self.homes[home_idx];
        home.master_key = Some(master_key);
        home.state = HomeState::Unlocked;
        home.last_unlock = Some(crate::time::uptime_ms());
        home.mount_count += 1;

        STATS.unlocks.fetch_add(1, Ordering::Relaxed);
        kprintln!("encrypted-home: Unlocked home for uid {}", uid);

        Ok(())
    }

    /// Decrypt key
    fn decrypt_key(
        &self,
        encrypted: &[u8],
        _key: &[u8; 32],
        _nonce: &[u8; 12],
        _tag: &[u8; 16],
    ) -> EncryptedHomeResult<[u8; 64]> {
        // Placeholder - would use AES-GCM decryption
        let mut result = [0u8; 64];
        if encrypted.len() >= 64 {
            result.copy_from_slice(&encrypted[..64]);
        }
        Ok(result)
    }

    /// Mount encrypted filesystem
    fn mount_encrypted(
        &self,
        config: &EncryptedHomeConfig,
        _master_key: &[u8; 64],
    ) -> EncryptedHomeResult<()> {
        kprintln!("encrypted-home: Mounting {} at {}", config.home_path, config.mount_point);
        // Would mount the encrypted filesystem
        Ok(())
    }

    /// Unlock with TPM
    pub fn unlock_with_tpm(&mut self, uid: u32) -> EncryptedHomeResult<()> {
        kprintln!("encrypted-home: TPM unlock for uid {}", uid);

        if !self.tpm_available {
            return Err(EncryptedHomeError::TpmError);
        }

        let home_idx = self.homes.iter().position(|h| h.config.uid == uid)
            .ok_or(EncryptedHomeError::NotFound)?;

        let home = &self.homes[home_idx];

        if home.state == HomeState::Unlocked {
            return Err(EncryptedHomeError::AlreadyUnlocked);
        }

        // Find TPM wrapped key
        let wrapped = home.wrapped_keys.iter()
            .find(|k| k.key_type == WrappedKeyType::Tpm)
            .ok_or(EncryptedHomeError::NotFound)?;

        // Unseal from TPM
        let master_key = self.unseal_from_tpm(wrapped)?;

        // Mount
        let config = home.config.clone();
        self.mount_encrypted(&config, &master_key)?;

        let home = &mut self.homes[home_idx];
        home.master_key = Some(master_key);
        home.state = HomeState::Unlocked;
        home.last_unlock = Some(crate::time::uptime_ms());
        home.mount_count += 1;

        STATS.unlocks.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }

    /// Unseal from TPM
    fn unseal_from_tpm(&self, wrapped: &WrappedKey) -> EncryptedHomeResult<[u8; 64]> {
        // Would unseal from TPM
        let mut result = [0u8; 64];
        if wrapped.encrypted_key.len() >= 64 {
            result.copy_from_slice(&wrapped.encrypted_key[..64]);
        }
        Ok(result)
    }

    /// Unlock with recovery key
    pub fn unlock_with_recovery(&mut self, uid: u32, recovery_phrase: &str) -> EncryptedHomeResult<()> {
        kprintln!("encrypted-home: Recovery unlock for uid {}", uid);

        let home_idx = self.homes.iter().position(|h| h.config.uid == uid)
            .ok_or(EncryptedHomeError::NotFound)?;

        let home = &self.homes[home_idx];

        if home.state == HomeState::Unlocked {
            return Err(EncryptedHomeError::AlreadyUnlocked);
        }

        // Find recovery wrapped key
        let wrapped = home.wrapped_keys.iter()
            .find(|k| k.key_type == WrappedKeyType::Recovery)
            .ok_or(EncryptedHomeError::NoRecoveryKey)?;

        // Derive key from recovery phrase
        let derived = self.derive_key(
            recovery_phrase,
            &wrapped.salt,
            &KeyDerivation::Pbkdf2,
            &KdfParams::default(),
        )?;

        // Decrypt master key
        let master_key = self.decrypt_key(&wrapped.encrypted_key, &derived, &wrapped.nonce, &wrapped.tag)?;

        // Mount
        let config = home.config.clone();
        self.mount_encrypted(&config, &master_key)?;

        let home = &mut self.homes[home_idx];
        home.master_key = Some(master_key);
        home.state = HomeState::Unlocked;
        home.last_unlock = Some(crate::time::uptime_ms());
        home.mount_count += 1;

        STATS.unlocks.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }

    /// Lock encrypted home
    pub fn lock(&mut self, uid: u32) -> EncryptedHomeResult<()> {
        kprintln!("encrypted-home: Locking home for uid {}", uid);

        // First check state and get config with immutable borrow
        let (state, config_clone) = {
            let home = self.homes.iter().find(|h| h.config.uid == uid)
                .ok_or(EncryptedHomeError::NotFound)?;
            (home.state, home.config.clone())
        };

        if state == HomeState::Locked {
            return Err(EncryptedHomeError::AlreadyLocked);
        }

        if state != HomeState::Unlocked {
            return Err(EncryptedHomeError::InvalidState);
        }

        // Unmount
        self.unmount_encrypted(&config_clone)?;

        // Clear key and set state
        let home = self.homes.iter_mut().find(|h| h.config.uid == uid)
            .ok_or(EncryptedHomeError::NotFound)?;
        home.clear_key();
        home.state = HomeState::Locked;

        STATS.locks.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }

    /// Unmount encrypted filesystem
    fn unmount_encrypted(&self, config: &EncryptedHomeConfig) -> EncryptedHomeResult<()> {
        kprintln!("encrypted-home: Unmounting {}", config.mount_point);
        // Would unmount
        Ok(())
    }

    /// Change password
    pub fn change_password(
        &mut self,
        uid: u32,
        old_password: &str,
        new_password: &str,
    ) -> EncryptedHomeResult<()> {
        kprintln!("encrypted-home: Changing password for uid {}", uid);

        // Check if we need to unlock first
        let state = self.homes.iter()
            .find(|h| h.config.uid == uid)
            .ok_or(EncryptedHomeError::NotFound)?
            .state;

        // Must be unlocked to change password (we need the master key)
        if state != HomeState::Unlocked {
            // Try to unlock first
            self.unlock(uid, old_password)?;
        }

        // Extract what we need with immutable borrow
        let (master_key, kdf, kdf_params) = {
            let home = self.homes.iter()
                .find(|h| h.config.uid == uid)
                .ok_or(EncryptedHomeError::NotFound)?;
            let mk = home.master_key.ok_or(EncryptedHomeError::InvalidState)?;
            (mk, home.config.kdf, home.config.kdf_params.clone())
        };

        // Create new wrapped key
        let new_wrapped = self.wrap_key_with_password(
            &master_key,
            new_password,
            &kdf,
            &kdf_params,
        )?;

        // Replace password wrapped key with mutable borrow
        let home = self.homes.iter_mut().find(|h| h.config.uid == uid)
            .ok_or(EncryptedHomeError::NotFound)?;
        home.wrapped_keys.retain(|k| k.key_type != WrappedKeyType::Password);
        home.wrapped_keys.push(new_wrapped);

        self.save_configs();
        Ok(())
    }

    /// Migrate existing home to encrypted
    pub fn migrate_to_encrypted(
        &mut self,
        uid: u32,
        username: &str,
        password: &str,
    ) -> EncryptedHomeResult<String> {
        kprintln!("encrypted-home: Migrating home for {}", username);
        STATS.migrations_started.fetch_add(1, Ordering::Relaxed);

        // Set up encryption
        let recovery = self.setup(uid, username, password, SetupOptions::default())?;

        // Would copy existing data to encrypted storage
        // ...

        STATS.migrations_completed.fetch_add(1, Ordering::Relaxed);
        Ok(recovery)
    }

    /// Get home state
    pub fn get_state(&self, uid: u32) -> Option<HomeState> {
        self.homes.iter()
            .find(|h| h.config.uid == uid)
            .map(|h| h.state)
    }

    /// Get home config
    pub fn get_config(&self, uid: u32) -> Option<&EncryptedHomeConfig> {
        self.homes.iter()
            .find(|h| h.config.uid == uid)
            .map(|h| &h.config)
    }

    /// Check if home is encrypted
    pub fn is_encrypted(&self, uid: u32) -> bool {
        self.homes.iter()
            .any(|h| h.config.uid == uid && h.state != HomeState::Unencrypted)
    }

    /// Check if home is unlocked
    pub fn is_unlocked(&self, uid: u32) -> bool {
        self.homes.iter()
            .any(|h| h.config.uid == uid && h.state == HomeState::Unlocked)
    }

    /// List all encrypted homes
    pub fn list_homes(&self) -> Vec<(u32, String, HomeState)> {
        self.homes.iter()
            .map(|h| (h.config.uid, h.config.username.clone(), h.state))
            .collect()
    }

    /// Format status
    pub fn format_status(&self) -> String {
        use alloc::fmt::Write;
        let mut s = String::new();

        let _ = writeln!(s, "Encrypted Home Status:");
        let _ = writeln!(s, "  TPM Available: {}", self.tpm_available);
        let _ = writeln!(s, "  Default Method: {}", self.default_method.as_str());
        let _ = writeln!(s, "  Default Cipher: {}", self.default_cipher.as_str());
        let _ = writeln!(s, "  Encrypted Homes: {}", self.homes.len());

        for home in &self.homes {
            let _ = writeln!(s, "\n  User: {} (uid {})", home.config.username, home.config.uid);
            let _ = writeln!(s, "    State: {}", home.state.as_str());
            let _ = writeln!(s, "    Method: {}", home.config.method.as_str());
            let _ = writeln!(s, "    Mount Point: {}", home.config.mount_point);
            let _ = writeln!(s, "    Key Types: {:?}",
                home.wrapped_keys.iter().map(|k| match k.key_type {
                    WrappedKeyType::Password => "password",
                    WrappedKeyType::Tpm => "tpm",
                    WrappedKeyType::Recovery => "recovery",
                    WrappedKeyType::Escrow => "escrow",
                }).collect::<Vec<_>>());
        }

        s
    }
}

impl Default for EncryptedHomeManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Setup options
#[derive(Debug, Clone, Default)]
pub struct SetupOptions {
    pub method: Option<EncryptionMethod>,
    pub cipher: Option<CipherAlgorithm>,
    pub kdf: Option<KeyDerivation>,
    pub kdf_params: Option<KdfParams>,
    pub auto_unlock: bool,
    pub require_password: bool,
    pub key_escrow: bool,
    pub use_tpm: bool,
    pub create_recovery: bool,
}

/// Recovery word list (BIP39 subset)
const RECOVERY_WORDS: &[&str] = &[
    "abandon", "ability", "able", "about", "above", "absent", "absorb", "abstract",
    "absurd", "abuse", "access", "accident", "account", "accuse", "achieve", "acid",
    "acoustic", "acquire", "across", "act", "action", "actor", "actress", "actual",
    "adapt", "add", "addict", "address", "adjust", "admit", "adult", "advance",
    "advice", "aerobic", "affair", "afford", "afraid", "again", "age", "agent",
    "agree", "ahead", "aim", "air", "airport", "aisle", "alarm", "album",
    "alcohol", "alert", "alien", "all", "alley", "allow", "almost", "alone",
    "alpha", "already", "also", "alter", "always", "amateur", "amazing", "among",
];

// === Public API ===

/// Initialize encrypted home
pub fn init() {
    let mut guard = ENCRYPTED_HOME.lock();
    if guard.is_none() {
        let mut manager = EncryptedHomeManager::new();
        manager.init();
        *guard = Some(manager);
    }
}

/// Set up encrypted home
pub fn setup(
    uid: u32,
    username: &str,
    password: &str,
    options: SetupOptions,
) -> EncryptedHomeResult<String> {
    ENCRYPTED_HOME.lock().as_mut()
        .expect("Not initialized")
        .setup(uid, username, password, options)
}

/// Unlock encrypted home
pub fn unlock(uid: u32, password: &str) -> EncryptedHomeResult<()> {
    ENCRYPTED_HOME.lock().as_mut()
        .expect("Not initialized")
        .unlock(uid, password)
}

/// Unlock with TPM
pub fn unlock_with_tpm(uid: u32) -> EncryptedHomeResult<()> {
    ENCRYPTED_HOME.lock().as_mut()
        .expect("Not initialized")
        .unlock_with_tpm(uid)
}

/// Unlock with recovery
pub fn unlock_with_recovery(uid: u32, recovery: &str) -> EncryptedHomeResult<()> {
    ENCRYPTED_HOME.lock().as_mut()
        .expect("Not initialized")
        .unlock_with_recovery(uid, recovery)
}

/// Lock encrypted home
pub fn lock(uid: u32) -> EncryptedHomeResult<()> {
    ENCRYPTED_HOME.lock().as_mut()
        .expect("Not initialized")
        .lock(uid)
}

/// Change password
pub fn change_password(uid: u32, old: &str, new: &str) -> EncryptedHomeResult<()> {
    ENCRYPTED_HOME.lock().as_mut()
        .expect("Not initialized")
        .change_password(uid, old, new)
}

/// Check if encrypted
pub fn is_encrypted(uid: u32) -> bool {
    ENCRYPTED_HOME.lock().as_ref()
        .map(|m| m.is_encrypted(uid))
        .unwrap_or(false)
}

/// Check if unlocked
pub fn is_unlocked(uid: u32) -> bool {
    ENCRYPTED_HOME.lock().as_ref()
        .map(|m| m.is_unlocked(uid))
        .unwrap_or(false)
}

/// Get state
pub fn get_state(uid: u32) -> Option<HomeState> {
    ENCRYPTED_HOME.lock().as_ref()
        .and_then(|m| m.get_state(uid))
}

/// Get statistics
pub fn stats() -> (u64, u64, u64) {
    (
        STATS.homes_encrypted.load(Ordering::Relaxed),
        STATS.unlocks.load(Ordering::Relaxed),
        STATS.locks.load(Ordering::Relaxed),
    )
}

/// Format status
pub fn format_status() -> String {
    ENCRYPTED_HOME.lock().as_ref()
        .map(|m| m.format_status())
        .unwrap_or_else(|| "Encrypted Home: Not initialized".to_string())
}
