//! Cryptographic Primitives
//!
//! Provides cryptographic functions for TLS and other security features:
//! - SHA-256 hash function and HMAC
//! - ChaCha20-Poly1305 AEAD cipher
//! - X25519 key exchange
//! - Ed25519 digital signatures
//! - RSA encryption and signatures

#![allow(dead_code)]

pub mod aes;
pub mod sha256;
pub mod chacha20;
pub mod x25519;
pub mod ed25519;
pub mod rsa;
pub mod random;
pub mod luks;

// Re-export commonly used items
pub use sha256::{sha256, hmac_sha256, hkdf_extract, hkdf_expand, Sha256, Sha256Digest};
pub use chacha20::{chacha20_encrypt, chacha20_poly1305_encrypt, chacha20_poly1305_decrypt};
pub use x25519::{x25519, x25519_public_key, x25519_diffie_hellman};
pub use ed25519::{Keypair as Ed25519Keypair, sign as ed25519_sign, verify as ed25519_verify, public_key_from_secret as ed25519_public_key};
pub use rsa::{RsaPublicKey, RsaPrivateKey, generate_keypair as rsa_generate_keypair};

/// Generate random bytes using the kernel's PRNG
pub fn random_bytes(out: &mut [u8]) {
    for byte in out.iter_mut() {
        *byte = crate::fs::devfs::random_byte();
    }
}

/// Generate a random 32-byte key
pub fn random_key() -> [u8; 32] {
    let mut key = [0u8; 32];
    random_bytes(&mut key);
    key
}

/// Constant-time comparison of byte slices
pub fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }

    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

/// Get a single random byte
pub fn random_byte() -> u8 {
    crate::fs::devfs::random_byte()
}

/// Initialize the crypto subsystem
pub fn init() {
    crate::kprintln!("crypto: SHA-256, ChaCha20-Poly1305, X25519, Ed25519, RSA available");
}
