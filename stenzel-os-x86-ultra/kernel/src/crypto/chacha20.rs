//! ChaCha20-Poly1305 AEAD
//!
//! Implementation of RFC 8439 ChaCha20 stream cipher and Poly1305 MAC.

#![allow(dead_code)]

use alloc::vec::Vec;

/// ChaCha20 quarter round
#[inline(always)]
fn quarter_round(state: &mut [u32; 16], a: usize, b: usize, c: usize, d: usize) {
    state[a] = state[a].wrapping_add(state[b]);
    state[d] ^= state[a];
    state[d] = state[d].rotate_left(16);

    state[c] = state[c].wrapping_add(state[d]);
    state[b] ^= state[c];
    state[b] = state[b].rotate_left(12);

    state[a] = state[a].wrapping_add(state[b]);
    state[d] ^= state[a];
    state[d] = state[d].rotate_left(8);

    state[c] = state[c].wrapping_add(state[d]);
    state[b] ^= state[c];
    state[b] = state[b].rotate_left(7);
}

/// ChaCha20 block function
fn chacha20_block(key: &[u8; 32], counter: u32, nonce: &[u8; 12]) -> [u8; 64] {
    // Initialize state
    let mut state: [u32; 16] = [
        // "expand 32-byte k"
        0x61707865,
        0x3320646e,
        0x79622d32,
        0x6b206574,
        // Key
        u32::from_le_bytes([key[0], key[1], key[2], key[3]]),
        u32::from_le_bytes([key[4], key[5], key[6], key[7]]),
        u32::from_le_bytes([key[8], key[9], key[10], key[11]]),
        u32::from_le_bytes([key[12], key[13], key[14], key[15]]),
        u32::from_le_bytes([key[16], key[17], key[18], key[19]]),
        u32::from_le_bytes([key[20], key[21], key[22], key[23]]),
        u32::from_le_bytes([key[24], key[25], key[26], key[27]]),
        u32::from_le_bytes([key[28], key[29], key[30], key[31]]),
        // Counter
        counter,
        // Nonce
        u32::from_le_bytes([nonce[0], nonce[1], nonce[2], nonce[3]]),
        u32::from_le_bytes([nonce[4], nonce[5], nonce[6], nonce[7]]),
        u32::from_le_bytes([nonce[8], nonce[9], nonce[10], nonce[11]]),
    ];

    let initial_state = state;

    // 20 rounds (10 double rounds)
    for _ in 0..10 {
        // Column rounds
        quarter_round(&mut state, 0, 4, 8, 12);
        quarter_round(&mut state, 1, 5, 9, 13);
        quarter_round(&mut state, 2, 6, 10, 14);
        quarter_round(&mut state, 3, 7, 11, 15);
        // Diagonal rounds
        quarter_round(&mut state, 0, 5, 10, 15);
        quarter_round(&mut state, 1, 6, 11, 12);
        quarter_round(&mut state, 2, 7, 8, 13);
        quarter_round(&mut state, 3, 4, 9, 14);
    }

    // Add initial state
    for i in 0..16 {
        state[i] = state[i].wrapping_add(initial_state[i]);
    }

    // Serialize to bytes
    let mut output = [0u8; 64];
    for (i, word) in state.iter().enumerate() {
        output[i * 4..(i + 1) * 4].copy_from_slice(&word.to_le_bytes());
    }
    output
}

/// ChaCha20 encryption/decryption (same operation for stream cipher)
pub fn chacha20_encrypt(key: &[u8; 32], nonce: &[u8; 12], counter: u32, plaintext: &[u8]) -> Vec<u8> {
    let mut ciphertext = Vec::with_capacity(plaintext.len());
    let mut block_counter = counter;

    for chunk in plaintext.chunks(64) {
        let keystream = chacha20_block(key, block_counter, nonce);
        for (i, &byte) in chunk.iter().enumerate() {
            ciphertext.push(byte ^ keystream[i]);
        }
        block_counter = block_counter.wrapping_add(1);
    }

    ciphertext
}

/// Poly1305 MAC state
struct Poly1305 {
    r: [u32; 5],
    h: [u32; 5],
    pad: [u32; 4],
}

impl Poly1305 {
    /// Create new Poly1305 with key
    fn new(key: &[u8; 32]) -> Self {
        // r = key[0..16] with certain bits clamped
        let mut r = [0u32; 5];
        r[0] = (u32::from_le_bytes([key[0], key[1], key[2], key[3]])) & 0x3ffffff;
        r[1] = (u32::from_le_bytes([key[3], key[4], key[5], key[6]]) >> 2) & 0x3ffff03;
        r[2] = (u32::from_le_bytes([key[6], key[7], key[8], key[9]]) >> 4) & 0x3ffc0ff;
        r[3] = (u32::from_le_bytes([key[9], key[10], key[11], key[12]]) >> 6) & 0x3f03fff;
        r[4] = (u32::from_le_bytes([key[12], key[13], key[14], key[15]]) >> 8) & 0x00fffff;

        // pad = key[16..32]
        let pad = [
            u32::from_le_bytes([key[16], key[17], key[18], key[19]]),
            u32::from_le_bytes([key[20], key[21], key[22], key[23]]),
            u32::from_le_bytes([key[24], key[25], key[26], key[27]]),
            u32::from_le_bytes([key[28], key[29], key[30], key[31]]),
        ];

        Self {
            r,
            h: [0; 5],
            pad,
        }
    }

    /// Add a 16-byte block
    fn block(&mut self, data: &[u8], is_last: bool) {
        // Parse block into 5 26-bit limbs
        let hibit = if is_last { 0 } else { 1u32 << 24 };

        let mut n = [0u8; 17];
        let len = core::cmp::min(16, data.len());
        n[..len].copy_from_slice(&data[..len]);
        if !is_last && len < 16 {
            n[len] = 1;
        }

        let t0 = u32::from_le_bytes([n[0], n[1], n[2], n[3]]);
        let t1 = u32::from_le_bytes([n[4], n[5], n[6], n[7]]);
        let t2 = u32::from_le_bytes([n[8], n[9], n[10], n[11]]);
        let t3 = u32::from_le_bytes([n[12], n[13], n[14], n[15]]);

        self.h[0] = self.h[0].wrapping_add(t0 & 0x3ffffff);
        self.h[1] = self.h[1].wrapping_add(((t1 << 6) | (t0 >> 26)) & 0x3ffffff);
        self.h[2] = self.h[2].wrapping_add(((t2 << 12) | (t1 >> 20)) & 0x3ffffff);
        self.h[3] = self.h[3].wrapping_add(((t3 << 18) | (t2 >> 14)) & 0x3ffffff);
        self.h[4] = self.h[4].wrapping_add((t3 >> 8) | hibit);

        // Multiply by r
        let s1 = self.r[1] * 5;
        let s2 = self.r[2] * 5;
        let s3 = self.r[3] * 5;
        let s4 = self.r[4] * 5;

        let d0 = self.h[0] as u64 * self.r[0] as u64
            + self.h[1] as u64 * s4 as u64
            + self.h[2] as u64 * s3 as u64
            + self.h[3] as u64 * s2 as u64
            + self.h[4] as u64 * s1 as u64;

        let d1 = self.h[0] as u64 * self.r[1] as u64
            + self.h[1] as u64 * self.r[0] as u64
            + self.h[2] as u64 * s4 as u64
            + self.h[3] as u64 * s3 as u64
            + self.h[4] as u64 * s2 as u64;

        let d2 = self.h[0] as u64 * self.r[2] as u64
            + self.h[1] as u64 * self.r[1] as u64
            + self.h[2] as u64 * self.r[0] as u64
            + self.h[3] as u64 * s4 as u64
            + self.h[4] as u64 * s3 as u64;

        let d3 = self.h[0] as u64 * self.r[3] as u64
            + self.h[1] as u64 * self.r[2] as u64
            + self.h[2] as u64 * self.r[1] as u64
            + self.h[3] as u64 * self.r[0] as u64
            + self.h[4] as u64 * s4 as u64;

        let d4 = self.h[0] as u64 * self.r[4] as u64
            + self.h[1] as u64 * self.r[3] as u64
            + self.h[2] as u64 * self.r[2] as u64
            + self.h[3] as u64 * self.r[1] as u64
            + self.h[4] as u64 * self.r[0] as u64;

        // Partial reduction mod 2^130-5
        let mut c: u32;
        c = (d0 >> 26) as u32;
        self.h[0] = (d0 as u32) & 0x3ffffff;
        let d1 = d1 + c as u64;

        c = (d1 >> 26) as u32;
        self.h[1] = (d1 as u32) & 0x3ffffff;
        let d2 = d2 + c as u64;

        c = (d2 >> 26) as u32;
        self.h[2] = (d2 as u32) & 0x3ffffff;
        let d3 = d3 + c as u64;

        c = (d3 >> 26) as u32;
        self.h[3] = (d3 as u32) & 0x3ffffff;
        let d4 = d4 + c as u64;

        c = (d4 >> 26) as u32;
        self.h[4] = (d4 as u32) & 0x3ffffff;
        self.h[0] = self.h[0].wrapping_add(c * 5);

        c = self.h[0] >> 26;
        self.h[0] &= 0x3ffffff;
        self.h[1] = self.h[1].wrapping_add(c);
    }

    /// Finalize and return the tag
    fn finalize(mut self) -> [u8; 16] {
        // Full carry chain
        let mut c = self.h[1] >> 26;
        self.h[1] &= 0x3ffffff;
        self.h[2] = self.h[2].wrapping_add(c);

        c = self.h[2] >> 26;
        self.h[2] &= 0x3ffffff;
        self.h[3] = self.h[3].wrapping_add(c);

        c = self.h[3] >> 26;
        self.h[3] &= 0x3ffffff;
        self.h[4] = self.h[4].wrapping_add(c);

        c = self.h[4] >> 26;
        self.h[4] &= 0x3ffffff;
        self.h[0] = self.h[0].wrapping_add(c * 5);

        c = self.h[0] >> 26;
        self.h[0] &= 0x3ffffff;
        self.h[1] = self.h[1].wrapping_add(c);

        // Compute h - p
        let mut g0 = self.h[0].wrapping_add(5);
        c = g0 >> 26;
        g0 &= 0x3ffffff;

        let mut g1 = self.h[1].wrapping_add(c);
        c = g1 >> 26;
        g1 &= 0x3ffffff;

        let mut g2 = self.h[2].wrapping_add(c);
        c = g2 >> 26;
        g2 &= 0x3ffffff;

        let mut g3 = self.h[3].wrapping_add(c);
        c = g3 >> 26;
        g3 &= 0x3ffffff;

        let g4 = self.h[4].wrapping_add(c).wrapping_sub(1 << 26);

        // Select h if h < p, or h - p if h >= p
        let mask = (g4 >> 31).wrapping_sub(1);
        g0 &= mask;
        g1 &= mask;
        g2 &= mask;
        g3 &= mask;
        let g4 = g4 & mask;

        let mask = !mask;
        self.h[0] = (self.h[0] & mask) | g0;
        self.h[1] = (self.h[1] & mask) | g1;
        self.h[2] = (self.h[2] & mask) | g2;
        self.h[3] = (self.h[3] & mask) | g3;
        self.h[4] = (self.h[4] & mask) | g4;

        // h = h mod 2^128
        let h0 = (self.h[0] | (self.h[1] << 26)) as u64;
        let h1 = ((self.h[1] >> 6) | (self.h[2] << 20)) as u64;
        let h2 = ((self.h[2] >> 12) | (self.h[3] << 14)) as u64;
        let h3 = ((self.h[3] >> 18) | (self.h[4] << 8)) as u64;

        // mac = (h + pad) mod 2^128
        let mut f: u64;
        f = h0 + self.pad[0] as u64;
        let h0 = f as u32;
        f = h1 + self.pad[1] as u64 + (f >> 32);
        let h1 = f as u32;
        f = h2 + self.pad[2] as u64 + (f >> 32);
        let h2 = f as u32;
        f = h3 + self.pad[3] as u64 + (f >> 32);
        let h3 = f as u32;

        let mut tag = [0u8; 16];
        tag[0..4].copy_from_slice(&h0.to_le_bytes());
        tag[4..8].copy_from_slice(&h1.to_le_bytes());
        tag[8..12].copy_from_slice(&h2.to_le_bytes());
        tag[12..16].copy_from_slice(&h3.to_le_bytes());

        tag
    }
}

/// Poly1305 MAC computation
pub fn poly1305(key: &[u8; 32], message: &[u8]) -> [u8; 16] {
    let mut mac = Poly1305::new(key);

    let mut offset = 0;
    while offset + 16 <= message.len() {
        mac.block(&message[offset..offset + 16], false);
        offset += 16;
    }

    if offset < message.len() {
        let remaining = &message[offset..];
        let mut padded = [0u8; 16];
        padded[..remaining.len()].copy_from_slice(remaining);
        padded[remaining.len()] = 1;
        mac.block(&padded, true);
    }

    mac.finalize()
}

/// Pad16 - pad to 16-byte boundary
fn pad16(len: usize) -> usize {
    if len % 16 == 0 {
        0
    } else {
        16 - (len % 16)
    }
}

/// ChaCha20-Poly1305 AEAD encryption (RFC 8439)
pub fn chacha20_poly1305_encrypt(
    key: &[u8; 32],
    nonce: &[u8; 12],
    aad: &[u8],
    plaintext: &[u8],
) -> (Vec<u8>, [u8; 16]) {
    // Encrypt
    let ciphertext = chacha20_encrypt(key, nonce, 1, plaintext);

    // Generate Poly1305 key
    let poly_key_block = chacha20_block(key, 0, nonce);
    let poly_key: [u8; 32] = poly_key_block[..32].try_into().unwrap();

    // Construct Poly1305 input
    let mut mac_data = Vec::new();
    mac_data.extend_from_slice(aad);
    mac_data.resize(mac_data.len() + pad16(aad.len()), 0);
    mac_data.extend_from_slice(&ciphertext);
    mac_data.resize(mac_data.len() + pad16(ciphertext.len()), 0);
    mac_data.extend_from_slice(&(aad.len() as u64).to_le_bytes());
    mac_data.extend_from_slice(&(ciphertext.len() as u64).to_le_bytes());

    let tag = poly1305(&poly_key, &mac_data);

    (ciphertext, tag)
}

/// ChaCha20-Poly1305 AEAD decryption (RFC 8439)
pub fn chacha20_poly1305_decrypt(
    key: &[u8; 32],
    nonce: &[u8; 12],
    aad: &[u8],
    ciphertext: &[u8],
    tag: &[u8; 16],
) -> Option<Vec<u8>> {
    // Generate Poly1305 key
    let poly_key_block = chacha20_block(key, 0, nonce);
    let poly_key: [u8; 32] = poly_key_block[..32].try_into().unwrap();

    // Construct Poly1305 input
    let mut mac_data = Vec::new();
    mac_data.extend_from_slice(aad);
    mac_data.resize(mac_data.len() + pad16(aad.len()), 0);
    mac_data.extend_from_slice(ciphertext);
    mac_data.resize(mac_data.len() + pad16(ciphertext.len()), 0);
    mac_data.extend_from_slice(&(aad.len() as u64).to_le_bytes());
    mac_data.extend_from_slice(&(ciphertext.len() as u64).to_le_bytes());

    let expected_tag = poly1305(&poly_key, &mac_data);

    // Constant-time comparison
    let mut diff = 0u8;
    for i in 0..16 {
        diff |= tag[i] ^ expected_tag[i];
    }

    if diff != 0 {
        return None; // Authentication failed
    }

    // Decrypt
    Some(chacha20_encrypt(key, nonce, 1, ciphertext))
}
