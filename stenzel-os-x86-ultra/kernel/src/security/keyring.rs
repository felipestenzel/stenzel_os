//! Kernel Keyring - Secure Key Storage
//!
//! Linux-compatible kernel keyring for storing cryptographic keys,
//! passwords, authentication tokens, and other security-sensitive data.

#![allow(dead_code)]

use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicU32, AtomicU64, Ordering};

use crate::process::Pid;
use super::{Uid, Gid};
use crate::sync::IrqSafeMutex;

/// Key serial number type
pub type KeySerial = i32;

/// Special key serial numbers
pub const KEY_SPEC_THREAD_KEYRING: KeySerial = -1;
pub const KEY_SPEC_PROCESS_KEYRING: KeySerial = -2;
pub const KEY_SPEC_SESSION_KEYRING: KeySerial = -3;
pub const KEY_SPEC_USER_KEYRING: KeySerial = -4;
pub const KEY_SPEC_USER_SESSION_KEYRING: KeySerial = -5;
pub const KEY_SPEC_GROUP_KEYRING: KeySerial = -6;
pub const KEY_SPEC_REQKEY_AUTH_KEY: KeySerial = -7;
pub const KEY_SPEC_REQUESTOR_KEYRING: KeySerial = -8;

/// Key permissions
pub const KEY_POS_VIEW: u32 = 0x01000000;
pub const KEY_POS_READ: u32 = 0x02000000;
pub const KEY_POS_WRITE: u32 = 0x04000000;
pub const KEY_POS_SEARCH: u32 = 0x08000000;
pub const KEY_POS_LINK: u32 = 0x10000000;
pub const KEY_POS_SETATTR: u32 = 0x20000000;
pub const KEY_POS_ALL: u32 = 0x3f000000;

pub const KEY_USR_VIEW: u32 = 0x00010000;
pub const KEY_USR_READ: u32 = 0x00020000;
pub const KEY_USR_WRITE: u32 = 0x00040000;
pub const KEY_USR_SEARCH: u32 = 0x00080000;
pub const KEY_USR_LINK: u32 = 0x00100000;
pub const KEY_USR_SETATTR: u32 = 0x00200000;
pub const KEY_USR_ALL: u32 = 0x003f0000;

pub const KEY_GRP_VIEW: u32 = 0x00000100;
pub const KEY_GRP_READ: u32 = 0x00000200;
pub const KEY_GRP_WRITE: u32 = 0x00000400;
pub const KEY_GRP_SEARCH: u32 = 0x00000800;
pub const KEY_GRP_LINK: u32 = 0x00001000;
pub const KEY_GRP_SETATTR: u32 = 0x00002000;
pub const KEY_GRP_ALL: u32 = 0x00003f00;

pub const KEY_OTH_VIEW: u32 = 0x00000001;
pub const KEY_OTH_READ: u32 = 0x00000002;
pub const KEY_OTH_WRITE: u32 = 0x00000004;
pub const KEY_OTH_SEARCH: u32 = 0x00000008;
pub const KEY_OTH_LINK: u32 = 0x00000010;
pub const KEY_OTH_SETATTR: u32 = 0x00000020;
pub const KEY_OTH_ALL: u32 = 0x0000003f;

/// Default permissions (owner all, user read/search, group search)
pub const KEY_DEFAULT_PERM: u32 = KEY_POS_ALL | KEY_USR_VIEW | KEY_USR_READ | KEY_USR_SEARCH | KEY_GRP_SEARCH;

/// Key types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyType {
    /// Raw key data (generic)
    User,
    /// Login credentials (user/password)
    Login,
    /// Keyring (holds other keys)
    Keyring,
    /// Big key (for large data, uses shmem)
    BigKey,
    /// Trusted key (TPM-based)
    Trusted,
    /// Encrypted key (requires master key)
    Encrypted,
    /// Logon key (similar to user but for kernel use)
    Logon,
    /// PKCS#7/CMS key
    Pkcs7,
    /// X.509 certificate
    X509,
    /// Asymmetric key
    Asymmetric,
    /// DNS resolver key
    DnsResolver,
    /// Request key authentication token
    RequestKeyAuth,
    /// Custom type
    Custom(u32),
}

impl KeyType {
    pub fn as_str(&self) -> &'static str {
        match self {
            KeyType::User => "user",
            KeyType::Login => "login",
            KeyType::Keyring => "keyring",
            KeyType::BigKey => "big_key",
            KeyType::Trusted => "trusted",
            KeyType::Encrypted => "encrypted",
            KeyType::Logon => "logon",
            KeyType::Pkcs7 => "pkcs7",
            KeyType::X509 => "x509",
            KeyType::Asymmetric => "asymmetric",
            KeyType::DnsResolver => "dns_resolver",
            KeyType::RequestKeyAuth => ".request_key_auth",
            KeyType::Custom(_) => "custom",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "user" => Some(KeyType::User),
            "login" => Some(KeyType::Login),
            "keyring" => Some(KeyType::Keyring),
            "big_key" => Some(KeyType::BigKey),
            "trusted" => Some(KeyType::Trusted),
            "encrypted" => Some(KeyType::Encrypted),
            "logon" => Some(KeyType::Logon),
            "pkcs7" => Some(KeyType::Pkcs7),
            "x509" => Some(KeyType::X509),
            "asymmetric" => Some(KeyType::Asymmetric),
            "dns_resolver" => Some(KeyType::DnsResolver),
            ".request_key_auth" => Some(KeyType::RequestKeyAuth),
            _ => None,
        }
    }
}

/// Key state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyState {
    /// Key is valid and usable
    Valid,
    /// Key is being constructed
    Construction,
    /// Key construction failed
    Negative,
    /// Key has been revoked
    Revoked,
    /// Key has expired
    Expired,
    /// Key data has been garbage collected
    Dead,
}

/// Key flags
#[derive(Debug, Clone, Copy)]
pub struct KeyFlags {
    /// Key is in quota
    pub quota_overrun: bool,
    /// Key is instantiated
    pub instantiated: bool,
    /// Key has been revoked
    pub revoked: bool,
    /// Key is dead (gc'd)
    pub dead: bool,
    /// Key cannot be invalidated
    pub no_invalidate: bool,
    /// Key construction is being retried
    pub retry: bool,
}

impl Default for KeyFlags {
    fn default() -> Self {
        Self {
            quota_overrun: false,
            instantiated: false,
            revoked: false,
            dead: false,
            no_invalidate: false,
            retry: false,
        }
    }
}

/// A kernel key
pub struct Key {
    /// Unique serial number
    pub serial: KeySerial,
    /// Key type
    pub key_type: KeyType,
    /// Key description (name)
    pub description: String,
    /// Key payload data
    pub payload: Vec<u8>,
    /// Owner UID
    pub uid: Uid,
    /// Owner GID
    pub gid: Gid,
    /// Permissions
    pub perm: u32,
    /// Current state
    pub state: KeyState,
    /// Flags
    pub flags: KeyFlags,
    /// Creation time (seconds since boot)
    pub ctime: u64,
    /// Last access time
    pub atime: u64,
    /// Expiration time (0 = never)
    pub expiry: u64,
    /// Reference count
    pub ref_count: AtomicU32,
    /// Linked keys (for keyrings)
    pub linked_keys: Vec<KeySerial>,
}

impl Key {
    pub fn new(
        serial: KeySerial,
        key_type: KeyType,
        description: &str,
        uid: Uid,
        gid: Gid,
    ) -> Self {
        let now = crate::time::uptime_secs();
        Self {
            serial,
            key_type,
            description: description.to_string(),
            payload: Vec::new(),
            uid,
            gid,
            perm: KEY_DEFAULT_PERM,
            state: KeyState::Construction,
            flags: KeyFlags::default(),
            ctime: now,
            atime: now,
            expiry: 0,
            ref_count: AtomicU32::new(1),
            linked_keys: Vec::new(),
        }
    }

    /// Set the key payload and mark as instantiated
    pub fn instantiate(&mut self, payload: Vec<u8>) {
        self.payload = payload;
        self.state = KeyState::Valid;
        self.flags.instantiated = true;
        self.atime = crate::time::uptime_secs();
    }

    /// Revoke the key
    pub fn revoke(&mut self) {
        self.state = KeyState::Revoked;
        self.flags.revoked = true;
    }

    /// Check if key is usable
    pub fn is_usable(&self) -> bool {
        matches!(self.state, KeyState::Valid)
    }

    /// Check if key has expired
    pub fn is_expired(&self) -> bool {
        if self.expiry == 0 {
            return false;
        }
        crate::time::uptime_secs() >= self.expiry
    }

    /// Update access time
    pub fn touch(&mut self) {
        self.atime = crate::time::uptime_secs();
    }

    /// Check permission
    pub fn check_permission(&self, uid: Uid, gid: Gid, perm: u32) -> bool {
        // Owner check
        if uid == self.uid {
            return (self.perm & (perm << 24)) != 0;
        }

        // Group check
        if gid == self.gid {
            return (self.perm & (perm << 8)) != 0;
        }

        // Other
        (self.perm & perm) != 0
    }

    /// Get reference
    pub fn get_ref(&self) {
        self.ref_count.fetch_add(1, Ordering::AcqRel);
    }

    /// Put reference
    pub fn put_ref(&self) -> u32 {
        self.ref_count.fetch_sub(1, Ordering::AcqRel) - 1
    }

    /// Get reference count
    pub fn refs(&self) -> u32 {
        self.ref_count.load(Ordering::Acquire)
    }
}

/// Error types for keyring operations
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyringError {
    NotFound,
    Exists,
    NoMemory,
    PermissionDenied,
    InvalidKey,
    InvalidKeyring,
    KeyRevoked,
    KeyExpired,
    QuotaExceeded,
    InvalidDescription,
    InvalidPayload,
}

/// Keyring manager
pub struct KeyringManager {
    /// All keys by serial number
    keys: IrqSafeMutex<BTreeMap<KeySerial, Arc<IrqSafeMutex<Key>>>>,
    /// Next serial number
    next_serial: AtomicU32,
    /// Per-user keyrings
    user_keyrings: IrqSafeMutex<BTreeMap<Uid, KeySerial>>,
    /// Per-user session keyrings
    user_session_keyrings: IrqSafeMutex<BTreeMap<Uid, KeySerial>>,
    /// Per-process keyrings
    process_keyrings: IrqSafeMutex<BTreeMap<Pid, KeySerial>>,
    /// Per-thread keyrings
    thread_keyrings: IrqSafeMutex<BTreeMap<(Pid, u32), KeySerial>>,
    /// Session keyrings (by session ID)
    session_keyrings: IrqSafeMutex<BTreeMap<u32, KeySerial>>,
}

impl KeyringManager {
    pub fn new() -> Self {
        Self {
            keys: IrqSafeMutex::new(BTreeMap::new()),
            next_serial: AtomicU32::new(1),
            user_keyrings: IrqSafeMutex::new(BTreeMap::new()),
            user_session_keyrings: IrqSafeMutex::new(BTreeMap::new()),
            process_keyrings: IrqSafeMutex::new(BTreeMap::new()),
            thread_keyrings: IrqSafeMutex::new(BTreeMap::new()),
            session_keyrings: IrqSafeMutex::new(BTreeMap::new()),
        }
    }

    /// Allocate a new serial number
    fn alloc_serial(&self) -> KeySerial {
        self.next_serial.fetch_add(1, Ordering::AcqRel) as KeySerial
    }

    /// Create a new key
    pub fn add_key(
        &self,
        key_type: KeyType,
        description: &str,
        payload: &[u8],
        dest_keyring: KeySerial,
        uid: Uid,
        gid: Gid,
    ) -> Result<KeySerial, KeyringError> {
        // Validate description
        if description.is_empty() || description.len() > 4096 {
            return Err(KeyringError::InvalidDescription);
        }

        // Create the key
        let serial = self.alloc_serial();
        let mut key = Key::new(serial, key_type, description, uid, gid);
        key.instantiate(payload.to_vec());

        // Add to global registry
        let key_arc = Arc::new(IrqSafeMutex::new(key));
        self.keys.lock().insert(serial, key_arc);

        // Link to destination keyring if specified
        if dest_keyring != 0 {
            self.link_key(serial, dest_keyring, uid, gid)?;
        }

        Ok(serial)
    }

    /// Create a new keyring
    pub fn add_keyring(
        &self,
        description: &str,
        dest_keyring: KeySerial,
        uid: Uid,
        gid: Gid,
    ) -> Result<KeySerial, KeyringError> {
        self.add_key(KeyType::Keyring, description, &[], dest_keyring, uid, gid)
    }

    /// Get a key by serial number
    pub fn get_key(&self, serial: KeySerial) -> Option<Arc<IrqSafeMutex<Key>>> {
        self.keys.lock().get(&serial).cloned()
    }

    /// Request a key by type and description
    pub fn request_key(
        &self,
        key_type: KeyType,
        description: &str,
        _dest_keyring: KeySerial,
        uid: Uid,
        gid: Gid,
    ) -> Result<KeySerial, KeyringError> {
        // Search for existing key
        let keys = self.keys.lock();
        for (serial, key_arc) in keys.iter() {
            let key = key_arc.lock();
            if key.key_type == key_type
                && key.description == description
                && key.is_usable()
                && key.check_permission(uid, gid, KEY_OTH_SEARCH)
            {
                return Ok(*serial);
            }
        }
        drop(keys);

        // Key not found - in a real implementation, this would invoke
        // /sbin/request-key to construct the key
        Err(KeyringError::NotFound)
    }

    /// Search for a key in a keyring
    pub fn search_keyring(
        &self,
        keyring_serial: KeySerial,
        key_type: KeyType,
        description: &str,
        uid: Uid,
        gid: Gid,
    ) -> Result<KeySerial, KeyringError> {
        let keyring = self.get_key(keyring_serial).ok_or(KeyringError::NotFound)?;
        let keyring_data = keyring.lock();

        if keyring_data.key_type != KeyType::Keyring {
            return Err(KeyringError::InvalidKeyring);
        }

        if !keyring_data.check_permission(uid, gid, KEY_OTH_SEARCH) {
            return Err(KeyringError::PermissionDenied);
        }

        // Search linked keys
        for &serial in &keyring_data.linked_keys {
            if let Some(key_arc) = self.keys.lock().get(&serial) {
                let key = key_arc.lock();
                if key.key_type == key_type
                    && key.description == description
                    && key.is_usable()
                {
                    return Ok(serial);
                }
            }
        }

        Err(KeyringError::NotFound)
    }

    /// Link a key to a keyring
    pub fn link_key(
        &self,
        key_serial: KeySerial,
        keyring_serial: KeySerial,
        uid: Uid,
        gid: Gid,
    ) -> Result<(), KeyringError> {
        let keyring = self.get_key(keyring_serial).ok_or(KeyringError::NotFound)?;
        let mut keyring_data = keyring.lock();

        if keyring_data.key_type != KeyType::Keyring {
            return Err(KeyringError::InvalidKeyring);
        }

        if !keyring_data.check_permission(uid, gid, KEY_OTH_WRITE) {
            return Err(KeyringError::PermissionDenied);
        }

        // Check key exists
        if self.get_key(key_serial).is_none() {
            return Err(KeyringError::NotFound);
        }

        // Add link if not already present
        if !keyring_data.linked_keys.contains(&key_serial) {
            keyring_data.linked_keys.push(key_serial);
        }

        Ok(())
    }

    /// Unlink a key from a keyring
    pub fn unlink_key(
        &self,
        key_serial: KeySerial,
        keyring_serial: KeySerial,
        uid: Uid,
        gid: Gid,
    ) -> Result<(), KeyringError> {
        let keyring = self.get_key(keyring_serial).ok_or(KeyringError::NotFound)?;
        let mut keyring_data = keyring.lock();

        if keyring_data.key_type != KeyType::Keyring {
            return Err(KeyringError::InvalidKeyring);
        }

        if !keyring_data.check_permission(uid, gid, KEY_OTH_WRITE) {
            return Err(KeyringError::PermissionDenied);
        }

        keyring_data.linked_keys.retain(|&s| s != key_serial);
        Ok(())
    }

    /// Read key payload
    pub fn read_key(
        &self,
        serial: KeySerial,
        uid: Uid,
        gid: Gid,
    ) -> Result<Vec<u8>, KeyringError> {
        let key = self.get_key(serial).ok_or(KeyringError::NotFound)?;
        let mut key_data = key.lock();

        if !key_data.check_permission(uid, gid, KEY_OTH_READ) {
            return Err(KeyringError::PermissionDenied);
        }

        if !key_data.is_usable() {
            if key_data.state == KeyState::Revoked {
                return Err(KeyringError::KeyRevoked);
            }
            return Err(KeyringError::InvalidKey);
        }

        key_data.touch();
        Ok(key_data.payload.clone())
    }

    /// Update key payload
    pub fn update_key(
        &self,
        serial: KeySerial,
        payload: &[u8],
        uid: Uid,
        gid: Gid,
    ) -> Result<(), KeyringError> {
        let key = self.get_key(serial).ok_or(KeyringError::NotFound)?;
        let mut key_data = key.lock();

        if !key_data.check_permission(uid, gid, KEY_OTH_WRITE) {
            return Err(KeyringError::PermissionDenied);
        }

        if key_data.state == KeyState::Revoked {
            return Err(KeyringError::KeyRevoked);
        }

        key_data.payload = payload.to_vec();
        key_data.touch();
        Ok(())
    }

    /// Revoke a key
    pub fn revoke_key(
        &self,
        serial: KeySerial,
        uid: Uid,
        gid: Gid,
    ) -> Result<(), KeyringError> {
        let key = self.get_key(serial).ok_or(KeyringError::NotFound)?;
        let mut key_data = key.lock();

        // Only owner can revoke
        if key_data.uid != uid {
            return Err(KeyringError::PermissionDenied);
        }

        key_data.revoke();
        Ok(())
    }

    /// Set key timeout
    pub fn set_timeout(
        &self,
        serial: KeySerial,
        timeout_secs: u64,
        uid: Uid,
        gid: Gid,
    ) -> Result<(), KeyringError> {
        let key = self.get_key(serial).ok_or(KeyringError::NotFound)?;
        let mut key_data = key.lock();

        if !key_data.check_permission(uid, gid, KEY_OTH_SETATTR) {
            return Err(KeyringError::PermissionDenied);
        }

        if timeout_secs == 0 {
            key_data.expiry = 0; // Never expires
        } else {
            key_data.expiry = crate::time::uptime_secs() + timeout_secs;
        }
        Ok(())
    }

    /// Set key permissions
    pub fn set_perm(
        &self,
        serial: KeySerial,
        perm: u32,
        uid: Uid,
        _gid: Gid,
    ) -> Result<(), KeyringError> {
        let key = self.get_key(serial).ok_or(KeyringError::NotFound)?;
        let mut key_data = key.lock();

        // Only owner can change permissions
        if key_data.uid != uid {
            return Err(KeyringError::PermissionDenied);
        }

        key_data.perm = perm;
        Ok(())
    }

    /// Get or create user keyring
    pub fn get_user_keyring(&self, uid: Uid, gid: Gid) -> KeySerial {
        let mut user_keyrings = self.user_keyrings.lock();

        if let Some(&serial) = user_keyrings.get(&uid) {
            return serial;
        }

        // Create new user keyring
        let description = alloc::format!("_uid.{}", uid.0);
        drop(user_keyrings);

        if let Ok(serial) = self.add_keyring(&description, 0, uid, gid) {
            self.user_keyrings.lock().insert(uid, serial);
            return serial;
        }

        0
    }

    /// Get or create user session keyring
    pub fn get_user_session_keyring(&self, uid: Uid, gid: Gid) -> KeySerial {
        let mut session_keyrings = self.user_session_keyrings.lock();

        if let Some(&serial) = session_keyrings.get(&uid) {
            return serial;
        }

        // Create new user session keyring
        let description = alloc::format!("_uid_ses.{}", uid.0);
        drop(session_keyrings);

        if let Ok(serial) = self.add_keyring(&description, 0, uid, gid) {
            self.user_session_keyrings.lock().insert(uid, serial);
            return serial;
        }

        0
    }

    /// Get or create process keyring
    pub fn get_process_keyring(&self, pid: Pid, uid: Uid, gid: Gid) -> KeySerial {
        let mut proc_keyrings = self.process_keyrings.lock();

        if let Some(&serial) = proc_keyrings.get(&pid) {
            return serial;
        }

        // Create new process keyring
        let description = alloc::format!("_pid.{}", pid.0);
        drop(proc_keyrings);

        if let Ok(serial) = self.add_keyring(&description, 0, uid, gid) {
            self.process_keyrings.lock().insert(pid, serial);
            return serial;
        }

        0
    }

    /// Clean up process keyring on exit
    pub fn cleanup_process(&self, pid: Pid) {
        if let Some(serial) = self.process_keyrings.lock().remove(&pid) {
            // Mark key as dead (will be gc'd)
            if let Some(key) = self.get_key(serial) {
                let mut key_data = key.lock();
                key_data.state = KeyState::Dead;
                key_data.flags.dead = true;
            }
        }
    }

    /// Describe a key
    pub fn describe_key(&self, serial: KeySerial, uid: Uid, gid: Gid) -> Result<String, KeyringError> {
        let key = self.get_key(serial).ok_or(KeyringError::NotFound)?;
        let key_data = key.lock();

        if !key_data.check_permission(uid, gid, KEY_OTH_VIEW) {
            return Err(KeyringError::PermissionDenied);
        }

        Ok(alloc::format!(
            "{};{};{};{:08x};{}",
            key_data.key_type.as_str(),
            key_data.uid.0,
            key_data.gid.0,
            key_data.perm,
            key_data.description
        ))
    }

    /// List keys in a keyring
    pub fn list_keyring(&self, keyring_serial: KeySerial, uid: Uid, gid: Gid) -> Result<Vec<KeySerial>, KeyringError> {
        let keyring = self.get_key(keyring_serial).ok_or(KeyringError::NotFound)?;
        let keyring_data = keyring.lock();

        if keyring_data.key_type != KeyType::Keyring {
            return Err(KeyringError::InvalidKeyring);
        }

        if !keyring_data.check_permission(uid, gid, KEY_OTH_READ) {
            return Err(KeyringError::PermissionDenied);
        }

        Ok(keyring_data.linked_keys.clone())
    }

    /// Resolve special keyring serial numbers
    pub fn resolve_special_keyring(
        &self,
        special: KeySerial,
        pid: Pid,
        _tid: u32,
        uid: Uid,
        gid: Gid,
        _sid: u32,
    ) -> Option<KeySerial> {
        match special {
            KEY_SPEC_THREAD_KEYRING => {
                // Thread keyring - not fully implemented
                None
            }
            KEY_SPEC_PROCESS_KEYRING => {
                Some(self.get_process_keyring(pid, uid, gid))
            }
            KEY_SPEC_SESSION_KEYRING => {
                // Would use session ID
                Some(self.get_user_session_keyring(uid, gid))
            }
            KEY_SPEC_USER_KEYRING => {
                Some(self.get_user_keyring(uid, gid))
            }
            KEY_SPEC_USER_SESSION_KEYRING => {
                Some(self.get_user_session_keyring(uid, gid))
            }
            _ => None,
        }
    }

    /// Garbage collect dead keys
    pub fn gc(&self) {
        let mut keys = self.keys.lock();
        let mut to_remove = Vec::new();

        for (serial, key_arc) in keys.iter() {
            let key = key_arc.lock();
            // Remove dead keys with no references
            if key.flags.dead && key.refs() == 1 {
                to_remove.push(*serial);
            }
            // Remove expired keys
            else if key.is_expired() {
                to_remove.push(*serial);
            }
        }

        for serial in to_remove {
            keys.remove(&serial);
        }
    }
}

impl Default for KeyringManager {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Global Instance
// ============================================================================

use spin::Once;

static KEYRING_MANAGER: Once<KeyringManager> = Once::new();

/// Initialize keyring subsystem
pub fn init() {
    KEYRING_MANAGER.call_once(KeyringManager::new);
    crate::kprintln!("keyring: Kernel keyring subsystem initialized");
}

/// Get the global keyring manager
pub fn manager() -> &'static KeyringManager {
    KEYRING_MANAGER.get().expect("keyring not initialized")
}

// ============================================================================
// Syscall Interface
// ============================================================================

use crate::util::KError;

/// Add a key to the kernel
pub fn sys_add_key(
    key_type: &str,
    description: &str,
    payload: &[u8],
    dest_keyring: KeySerial,
    uid: Uid,
    gid: Gid,
) -> Result<KeySerial, KError> {
    let ktype = KeyType::from_str(key_type).ok_or(KError::Invalid)?;

    manager()
        .add_key(ktype, description, payload, dest_keyring, uid, gid)
        .map_err(|e| match e {
            KeyringError::PermissionDenied => KError::PermissionDenied,
            KeyringError::NoMemory => KError::NoMemory,
            KeyringError::InvalidDescription => KError::Invalid,
            _ => KError::IO,
        })
}

/// Request a key from the kernel
pub fn sys_request_key(
    key_type: &str,
    description: &str,
    callout_info: Option<&str>,
    dest_keyring: KeySerial,
    uid: Uid,
    gid: Gid,
) -> Result<KeySerial, KError> {
    let ktype = KeyType::from_str(key_type).ok_or(KError::Invalid)?;
    let _ = callout_info; // Would be passed to /sbin/request-key

    manager()
        .request_key(ktype, description, dest_keyring, uid, gid)
        .map_err(|e| match e {
            KeyringError::NotFound => KError::NotFound,
            KeyringError::PermissionDenied => KError::PermissionDenied,
            _ => KError::IO,
        })
}

/// Read the payload of a key
pub fn sys_keyctl_read(serial: KeySerial, uid: Uid, gid: Gid) -> Result<Vec<u8>, KError> {
    manager()
        .read_key(serial, uid, gid)
        .map_err(|e| match e {
            KeyringError::NotFound => KError::NotFound,
            KeyringError::PermissionDenied => KError::PermissionDenied,
            KeyringError::KeyRevoked => KError::PermissionDenied,
            _ => KError::IO,
        })
}

/// Update the payload of a key
pub fn sys_keyctl_update(serial: KeySerial, payload: &[u8], uid: Uid, gid: Gid) -> Result<(), KError> {
    manager()
        .update_key(serial, payload, uid, gid)
        .map_err(|e| match e {
            KeyringError::NotFound => KError::NotFound,
            KeyringError::PermissionDenied => KError::PermissionDenied,
            KeyringError::KeyRevoked => KError::PermissionDenied,
            _ => KError::IO,
        })
}

/// Revoke a key
pub fn sys_keyctl_revoke(serial: KeySerial, uid: Uid, gid: Gid) -> Result<(), KError> {
    manager()
        .revoke_key(serial, uid, gid)
        .map_err(|e| match e {
            KeyringError::NotFound => KError::NotFound,
            KeyringError::PermissionDenied => KError::PermissionDenied,
            _ => KError::IO,
        })
}

/// Set key timeout
pub fn sys_keyctl_set_timeout(serial: KeySerial, timeout_secs: u64, uid: Uid, gid: Gid) -> Result<(), KError> {
    manager()
        .set_timeout(serial, timeout_secs, uid, gid)
        .map_err(|e| match e {
            KeyringError::NotFound => KError::NotFound,
            KeyringError::PermissionDenied => KError::PermissionDenied,
            _ => KError::IO,
        })
}

/// Set key permissions
pub fn sys_keyctl_setperm(serial: KeySerial, perm: u32, uid: Uid, gid: Gid) -> Result<(), KError> {
    manager()
        .set_perm(serial, perm, uid, gid)
        .map_err(|e| match e {
            KeyringError::NotFound => KError::NotFound,
            KeyringError::PermissionDenied => KError::PermissionDenied,
            _ => KError::IO,
        })
}

/// Describe a key
pub fn sys_keyctl_describe(serial: KeySerial, uid: Uid, gid: Gid) -> Result<String, KError> {
    manager()
        .describe_key(serial, uid, gid)
        .map_err(|e| match e {
            KeyringError::NotFound => KError::NotFound,
            KeyringError::PermissionDenied => KError::PermissionDenied,
            _ => KError::IO,
        })
}

/// Link a key to a keyring
pub fn sys_keyctl_link(key_serial: KeySerial, keyring_serial: KeySerial, uid: Uid, gid: Gid) -> Result<(), KError> {
    manager()
        .link_key(key_serial, keyring_serial, uid, gid)
        .map_err(|e| match e {
            KeyringError::NotFound => KError::NotFound,
            KeyringError::PermissionDenied => KError::PermissionDenied,
            KeyringError::InvalidKeyring => KError::Invalid,
            _ => KError::IO,
        })
}

/// Unlink a key from a keyring
pub fn sys_keyctl_unlink(key_serial: KeySerial, keyring_serial: KeySerial, uid: Uid, gid: Gid) -> Result<(), KError> {
    manager()
        .unlink_key(key_serial, keyring_serial, uid, gid)
        .map_err(|e| match e {
            KeyringError::NotFound => KError::NotFound,
            KeyringError::PermissionDenied => KError::PermissionDenied,
            KeyringError::InvalidKeyring => KError::Invalid,
            _ => KError::IO,
        })
}

/// Search for a key in a keyring
pub fn sys_keyctl_search(
    keyring_serial: KeySerial,
    key_type: &str,
    description: &str,
    uid: Uid,
    gid: Gid,
) -> Result<KeySerial, KError> {
    let ktype = KeyType::from_str(key_type).ok_or(KError::Invalid)?;

    manager()
        .search_keyring(keyring_serial, ktype, description, uid, gid)
        .map_err(|e| match e {
            KeyringError::NotFound => KError::NotFound,
            KeyringError::PermissionDenied => KError::PermissionDenied,
            KeyringError::InvalidKeyring => KError::Invalid,
            _ => KError::IO,
        })
}
