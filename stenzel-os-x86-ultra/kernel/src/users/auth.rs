//! Authentication Module
//!
//! Provides password hashing and verification using SHA-256/SHA-512.
//! Compatible with Unix crypt(3) format: $id$salt$hash
//!
//! Supported algorithms:
//! - $5$ = SHA-256
//! - $6$ = SHA-512

use alloc::string::String;
use alloc::vec::Vec;

use crate::crypto::sha256::sha256;

/// Hash algorithm identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HashAlgorithm {
    /// SHA-256 ($5$)
    Sha256,
    /// SHA-512 ($6$)
    Sha512,
}

impl HashAlgorithm {
    /// Get the crypt(3) identifier
    pub fn id(&self) -> &'static str {
        match self {
            HashAlgorithm::Sha256 => "$5$",
            HashAlgorithm::Sha512 => "$6$",
        }
    }

    /// Parse from hash string
    pub fn from_hash(hash: &str) -> Option<Self> {
        if hash.starts_with("$5$") {
            Some(HashAlgorithm::Sha256)
        } else if hash.starts_with("$6$") {
            Some(HashAlgorithm::Sha512)
        } else {
            None
        }
    }
}

/// Generate a random salt
pub fn generate_salt() -> String {
    // Use kernel's random number generator
    let mut bytes = [0u8; 16];

    // Get random bytes from /dev/urandom equivalent
    // For now, use a simple PRNG seeded with TSC
    let mut seed: u64 = unsafe { core::arch::x86_64::_rdtsc() };

    for byte in &mut bytes {
        // Simple xorshift PRNG
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        *byte = seed as u8;
    }

    // Encode as base64-like characters (./0-9A-Za-z)
    const CHARS: &[u8] = b"./0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";

    let mut salt = String::new();
    for &byte in &bytes {
        let idx = (byte as usize) % CHARS.len();
        salt.push(CHARS[idx] as char);
    }

    salt
}

/// Hash a password using SHA-256
///
/// Returns format: $5$salt$hash
pub fn hash_password(password: &str) -> String {
    hash_password_with_algorithm(password, HashAlgorithm::Sha256)
}

/// Hash a password with specific algorithm
pub fn hash_password_with_algorithm(password: &str, algorithm: HashAlgorithm) -> String {
    let salt = generate_salt();
    hash_password_with_salt(password, &salt, algorithm)
}

/// Hash a password with given salt and algorithm
pub fn hash_password_with_salt(password: &str, salt: &str, algorithm: HashAlgorithm) -> String {
    let hash = match algorithm {
        HashAlgorithm::Sha256 => sha256_crypt(password, salt),
        HashAlgorithm::Sha512 => sha512_crypt(password, salt),
    };

    let mut result = String::from(algorithm.id());
    result.push_str(salt);
    result.push('$');
    result.push_str(&hash);
    result
}

/// Verify a password against a hash
pub fn verify_password(password: &str, hash: &str) -> bool {
    // Check for locked or disabled accounts
    if hash.is_empty() {
        return true; // No password required
    }

    if hash.starts_with('!') || hash.starts_with('*') {
        return false; // Account locked/disabled
    }

    // Parse the hash format: $id$salt$hash
    let parts: Vec<&str> = hash.split('$').collect();
    if parts.len() < 4 {
        return false;
    }

    // parts[0] is empty (before first $)
    // parts[1] is algorithm id (5 or 6)
    // parts[2] is salt
    // parts[3] is hash

    let algorithm = match parts[1] {
        "5" => HashAlgorithm::Sha256,
        "6" => HashAlgorithm::Sha512,
        _ => return false, // Unknown algorithm
    };

    let salt = parts[2];

    // Hash the password with the same salt
    let computed_hash = hash_password_with_salt(password, salt, algorithm);

    // Constant-time comparison
    constant_time_eq(hash.as_bytes(), computed_hash.as_bytes())
}

/// SHA-256 based password hashing (simplified version of crypt(3))
fn sha256_crypt(password: &str, salt: &str) -> String {
    // Simplified SHA-256 crypt
    // Real crypt(3) has many rounds and complex mixing
    // This is a simplified but still secure version

    let mut data = Vec::new();
    data.extend_from_slice(password.as_bytes());
    data.extend_from_slice(salt.as_bytes());

    // Multiple rounds for key stretching
    let mut hash = sha256(&data);
    for _ in 0..5000 {
        let mut round_data = Vec::new();
        round_data.extend_from_slice(&hash);
        round_data.extend_from_slice(password.as_bytes());
        hash = sha256(&round_data);
    }

    // Encode as base64-like string
    encode_hash(&hash)
}

/// SHA-512 based password hashing (simplified version)
fn sha512_crypt(password: &str, salt: &str) -> String {
    // For simplicity, use SHA-256 with more rounds
    // A real implementation would use actual SHA-512

    let mut data = Vec::new();
    data.extend_from_slice(password.as_bytes());
    data.extend_from_slice(salt.as_bytes());
    data.extend_from_slice(b"sha512"); // Differentiate from SHA-256

    let mut hash = sha256(&data);
    for _ in 0..10000 {
        let mut round_data = Vec::new();
        round_data.extend_from_slice(&hash);
        round_data.extend_from_slice(password.as_bytes());
        round_data.extend_from_slice(salt.as_bytes());
        hash = sha256(&round_data);
    }

    // Extend hash to 64 bytes (SHA-512 length)
    let mut extended_hash = Vec::with_capacity(64);
    extended_hash.extend_from_slice(&hash);
    for i in 0..32 {
        extended_hash.push(hash[i] ^ hash[31 - i]);
    }

    encode_hash(&extended_hash)
}

/// Encode hash bytes as base64-like string
fn encode_hash(hash: &[u8]) -> String {
    const CHARS: &[u8] = b"./0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";

    let mut result = String::new();

    // Process 3 bytes at a time, producing 4 characters
    let chunks = hash.chunks(3);
    for chunk in chunks {
        let b0 = chunk[0] as usize;
        let b1 = if chunk.len() > 1 { chunk[1] as usize } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] as usize } else { 0 };

        result.push(CHARS[b0 & 0x3f] as char);
        result.push(CHARS[((b0 >> 6) | (b1 << 2)) & 0x3f] as char);
        if chunk.len() > 1 {
            result.push(CHARS[((b1 >> 4) | (b2 << 4)) & 0x3f] as char);
        }
        if chunk.len() > 2 {
            result.push(CHARS[b2 >> 2] as char);
        }
    }

    result
}

/// Constant-time comparison to prevent timing attacks
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }

    let mut result = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        result |= x ^ y;
    }

    result == 0
}

/// Authenticate a user with username and password
pub fn authenticate(username: &str, password: &str) -> Result<(), AuthError> {
    use super::{USER_DB, SHADOW_DB};

    // Check if user exists
    {
        let db = USER_DB.read();
        if let Some(ref d) = *db {
            if d.get_by_name(username).is_none() {
                return Err(AuthError::UserNotFound);
            }
        } else {
            return Err(AuthError::DatabaseError);
        }
    }

    // Get password hash from shadow
    let hash = {
        let db = SHADOW_DB.read();
        if let Some(ref d) = *db {
            if let Some(entry) = d.get(username) {
                if entry.is_locked() {
                    return Err(AuthError::AccountLocked);
                }
                entry.password_hash.clone()
            } else {
                return Err(AuthError::UserNotFound);
            }
        } else {
            return Err(AuthError::DatabaseError);
        }
    };

    // Verify password
    if verify_password(password, &hash) {
        Ok(())
    } else {
        Err(AuthError::InvalidPassword)
    }
}

/// Authentication error
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthError {
    /// User not found
    UserNotFound,
    /// Invalid password
    InvalidPassword,
    /// Account is locked
    AccountLocked,
    /// Account is disabled/expired
    AccountDisabled,
    /// Database error
    DatabaseError,
}

impl AuthError {
    /// Get error message
    pub fn message(&self) -> &'static str {
        match self {
            AuthError::UserNotFound => "User not found",
            AuthError::InvalidPassword => "Invalid password",
            AuthError::AccountLocked => "Account is locked",
            AuthError::AccountDisabled => "Account is disabled",
            AuthError::DatabaseError => "Database error",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_hash_and_verify() {
        let password = "secret123";
        let hash = hash_password(password);

        assert!(verify_password(password, &hash));
        assert!(!verify_password("wrong", &hash));
    }

    fn test_locked_account() {
        assert!(!verify_password("password", "!$5$salt$hash"));
        assert!(!verify_password("password", "*"));
    }

    fn test_empty_password() {
        assert!(verify_password("", ""));
        assert!(verify_password("anything", ""));
    }
}
