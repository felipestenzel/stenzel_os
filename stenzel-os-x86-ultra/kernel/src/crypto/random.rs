//! Cryptographic Random Number Generation
//!
//! Provides access to the kernel's random number generator for
//! cryptographic purposes.

#![allow(dead_code)]

/// Get a single random byte
pub fn get_random_u8() -> u8 {
    crate::fs::devfs::random_byte()
}

/// Get a random u16
pub fn get_random_u16() -> u16 {
    let b0 = get_random_u8() as u16;
    let b1 = get_random_u8() as u16;
    b0 | (b1 << 8)
}

/// Get a random u32
pub fn get_random_u32() -> u32 {
    let b0 = get_random_u8() as u32;
    let b1 = get_random_u8() as u32;
    let b2 = get_random_u8() as u32;
    let b3 = get_random_u8() as u32;
    b0 | (b1 << 8) | (b2 << 16) | (b3 << 24)
}

/// Get a random u64
pub fn get_random_u64() -> u64 {
    let low = get_random_u32() as u64;
    let high = get_random_u32() as u64;
    low | (high << 32)
}

/// Fill a buffer with random bytes
pub fn fill_random(buf: &mut [u8]) {
    for byte in buf.iter_mut() {
        *byte = get_random_u8();
    }
}

/// Generate a random 16-byte array
pub fn random_16() -> [u8; 16] {
    let mut buf = [0u8; 16];
    fill_random(&mut buf);
    buf
}

/// Generate a random 32-byte array
pub fn random_32() -> [u8; 32] {
    let mut buf = [0u8; 32];
    fill_random(&mut buf);
    buf
}

/// Generate a random 64-byte array
pub fn random_64() -> [u8; 64] {
    let mut buf = [0u8; 64];
    fill_random(&mut buf);
    buf
}

/// Generate a random number in range [0, max)
pub fn random_range(max: u32) -> u32 {
    if max == 0 {
        return 0;
    }
    get_random_u32() % max
}
