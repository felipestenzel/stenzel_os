//! AES (Advanced Encryption Standard) Implementation
//!
//! AES-128/192/256 encryption in ECB mode (building block for other modes).
//! Used by LUKS for AES-XTS disk encryption.

#![allow(dead_code)]

/// AES S-box
static SBOX: [u8; 256] = [
    0x63, 0x7c, 0x77, 0x7b, 0xf2, 0x6b, 0x6f, 0xc5, 0x30, 0x01, 0x67, 0x2b, 0xfe, 0xd7, 0xab, 0x76,
    0xca, 0x82, 0xc9, 0x7d, 0xfa, 0x59, 0x47, 0xf0, 0xad, 0xd4, 0xa2, 0xaf, 0x9c, 0xa4, 0x72, 0xc0,
    0xb7, 0xfd, 0x93, 0x26, 0x36, 0x3f, 0xf7, 0xcc, 0x34, 0xa5, 0xe5, 0xf1, 0x71, 0xd8, 0x31, 0x15,
    0x04, 0xc7, 0x23, 0xc3, 0x18, 0x96, 0x05, 0x9a, 0x07, 0x12, 0x80, 0xe2, 0xeb, 0x27, 0xb2, 0x75,
    0x09, 0x83, 0x2c, 0x1a, 0x1b, 0x6e, 0x5a, 0xa0, 0x52, 0x3b, 0xd6, 0xb3, 0x29, 0xe3, 0x2f, 0x84,
    0x53, 0xd1, 0x00, 0xed, 0x20, 0xfc, 0xb1, 0x5b, 0x6a, 0xcb, 0xbe, 0x39, 0x4a, 0x4c, 0x58, 0xcf,
    0xd0, 0xef, 0xaa, 0xfb, 0x43, 0x4d, 0x33, 0x85, 0x45, 0xf9, 0x02, 0x7f, 0x50, 0x3c, 0x9f, 0xa8,
    0x51, 0xa3, 0x40, 0x8f, 0x92, 0x9d, 0x38, 0xf5, 0xbc, 0xb6, 0xda, 0x21, 0x10, 0xff, 0xf3, 0xd2,
    0xcd, 0x0c, 0x13, 0xec, 0x5f, 0x97, 0x44, 0x17, 0xc4, 0xa7, 0x7e, 0x3d, 0x64, 0x5d, 0x19, 0x73,
    0x60, 0x81, 0x4f, 0xdc, 0x22, 0x2a, 0x90, 0x88, 0x46, 0xee, 0xb8, 0x14, 0xde, 0x5e, 0x0b, 0xdb,
    0xe0, 0x32, 0x3a, 0x0a, 0x49, 0x06, 0x24, 0x5c, 0xc2, 0xd3, 0xac, 0x62, 0x91, 0x95, 0xe4, 0x79,
    0xe7, 0xc8, 0x37, 0x6d, 0x8d, 0xd5, 0x4e, 0xa9, 0x6c, 0x56, 0xf4, 0xea, 0x65, 0x7a, 0xae, 0x08,
    0xba, 0x78, 0x25, 0x2e, 0x1c, 0xa6, 0xb4, 0xc6, 0xe8, 0xdd, 0x74, 0x1f, 0x4b, 0xbd, 0x8b, 0x8a,
    0x70, 0x3e, 0xb5, 0x66, 0x48, 0x03, 0xf6, 0x0e, 0x61, 0x35, 0x57, 0xb9, 0x86, 0xc1, 0x1d, 0x9e,
    0xe1, 0xf8, 0x98, 0x11, 0x69, 0xd9, 0x8e, 0x94, 0x9b, 0x1e, 0x87, 0xe9, 0xce, 0x55, 0x28, 0xdf,
    0x8c, 0xa1, 0x89, 0x0d, 0xbf, 0xe6, 0x42, 0x68, 0x41, 0x99, 0x2d, 0x0f, 0xb0, 0x54, 0xbb, 0x16,
];

/// AES inverse S-box
static INV_SBOX: [u8; 256] = [
    0x52, 0x09, 0x6a, 0xd5, 0x30, 0x36, 0xa5, 0x38, 0xbf, 0x40, 0xa3, 0x9e, 0x81, 0xf3, 0xd7, 0xfb,
    0x7c, 0xe3, 0x39, 0x82, 0x9b, 0x2f, 0xff, 0x87, 0x34, 0x8e, 0x43, 0x44, 0xc4, 0xde, 0xe9, 0xcb,
    0x54, 0x7b, 0x94, 0x32, 0xa6, 0xc2, 0x23, 0x3d, 0xee, 0x4c, 0x95, 0x0b, 0x42, 0xfa, 0xc3, 0x4e,
    0x08, 0x2e, 0xa1, 0x66, 0x28, 0xd9, 0x24, 0xb2, 0x76, 0x5b, 0xa2, 0x49, 0x6d, 0x8b, 0xd1, 0x25,
    0x72, 0xf8, 0xf6, 0x64, 0x86, 0x68, 0x98, 0x16, 0xd4, 0xa4, 0x5c, 0xcc, 0x5d, 0x65, 0xb6, 0x92,
    0x6c, 0x70, 0x48, 0x50, 0xfd, 0xed, 0xb9, 0xda, 0x5e, 0x15, 0x46, 0x57, 0xa7, 0x8d, 0x9d, 0x84,
    0x90, 0xd8, 0xab, 0x00, 0x8c, 0xbc, 0xd3, 0x0a, 0xf7, 0xe4, 0x58, 0x05, 0xb8, 0xb3, 0x45, 0x06,
    0xd0, 0x2c, 0x1e, 0x8f, 0xca, 0x3f, 0x0f, 0x02, 0xc1, 0xaf, 0xbd, 0x03, 0x01, 0x13, 0x8a, 0x6b,
    0x3a, 0x91, 0x11, 0x41, 0x4f, 0x67, 0xdc, 0xea, 0x97, 0xf2, 0xcf, 0xce, 0xf0, 0xb4, 0xe6, 0x73,
    0x96, 0xac, 0x74, 0x22, 0xe7, 0xad, 0x35, 0x85, 0xe2, 0xf9, 0x37, 0xe8, 0x1c, 0x75, 0xdf, 0x6e,
    0x47, 0xf1, 0x1a, 0x71, 0x1d, 0x29, 0xc5, 0x89, 0x6f, 0xb7, 0x62, 0x0e, 0xaa, 0x18, 0xbe, 0x1b,
    0xfc, 0x56, 0x3e, 0x4b, 0xc6, 0xd2, 0x79, 0x20, 0x9a, 0xdb, 0xc0, 0xfe, 0x78, 0xcd, 0x5a, 0xf4,
    0x1f, 0xdd, 0xa8, 0x33, 0x88, 0x07, 0xc7, 0x31, 0xb1, 0x12, 0x10, 0x59, 0x27, 0x80, 0xec, 0x5f,
    0x60, 0x51, 0x7f, 0xa9, 0x19, 0xb5, 0x4a, 0x0d, 0x2d, 0xe5, 0x7a, 0x9f, 0x93, 0xc9, 0x9c, 0xef,
    0xa0, 0xe0, 0x3b, 0x4d, 0xae, 0x2a, 0xf5, 0xb0, 0xc8, 0xeb, 0xbb, 0x3c, 0x83, 0x53, 0x99, 0x61,
    0x17, 0x2b, 0x04, 0x7e, 0xba, 0x77, 0xd6, 0x26, 0xe1, 0x69, 0x14, 0x63, 0x55, 0x21, 0x0c, 0x7d,
];

/// Round constants for key expansion
static RCON: [u8; 11] = [0x00, 0x01, 0x02, 0x04, 0x08, 0x10, 0x20, 0x40, 0x80, 0x1b, 0x36];

/// Galois field multiplication by 2
fn gf_mul2(x: u8) -> u8 {
    let h = (x >> 7) & 1;
    let shifted = x << 1;
    shifted ^ (h * 0x1b)
}

/// Galois field multiplication by 3
fn gf_mul3(x: u8) -> u8 {
    gf_mul2(x) ^ x
}

/// Galois field multiplication by 9
fn gf_mul9(x: u8) -> u8 {
    gf_mul2(gf_mul2(gf_mul2(x))) ^ x
}

/// Galois field multiplication by 11
fn gf_mul11(x: u8) -> u8 {
    gf_mul2(gf_mul2(gf_mul2(x)) ^ x) ^ x
}

/// Galois field multiplication by 13
fn gf_mul13(x: u8) -> u8 {
    gf_mul2(gf_mul2(gf_mul2(x) ^ x)) ^ x
}

/// Galois field multiplication by 14
fn gf_mul14(x: u8) -> u8 {
    gf_mul2(gf_mul2(gf_mul2(x) ^ x) ^ x)
}

/// SubBytes transformation
fn sub_bytes(state: &mut [u8; 16]) {
    for byte in state.iter_mut() {
        *byte = SBOX[*byte as usize];
    }
}

/// Inverse SubBytes transformation
fn inv_sub_bytes(state: &mut [u8; 16]) {
    for byte in state.iter_mut() {
        *byte = INV_SBOX[*byte as usize];
    }
}

/// ShiftRows transformation
fn shift_rows(state: &mut [u8; 16]) {
    // Row 0: no shift
    // Row 1: shift left by 1
    let tmp = state[1];
    state[1] = state[5];
    state[5] = state[9];
    state[9] = state[13];
    state[13] = tmp;

    // Row 2: shift left by 2
    let tmp1 = state[2];
    let tmp2 = state[6];
    state[2] = state[10];
    state[6] = state[14];
    state[10] = tmp1;
    state[14] = tmp2;

    // Row 3: shift left by 3 (= shift right by 1)
    let tmp = state[15];
    state[15] = state[11];
    state[11] = state[7];
    state[7] = state[3];
    state[3] = tmp;
}

/// Inverse ShiftRows transformation
fn inv_shift_rows(state: &mut [u8; 16]) {
    // Row 0: no shift
    // Row 1: shift right by 1
    let tmp = state[13];
    state[13] = state[9];
    state[9] = state[5];
    state[5] = state[1];
    state[1] = tmp;

    // Row 2: shift right by 2
    let tmp1 = state[2];
    let tmp2 = state[6];
    state[2] = state[10];
    state[6] = state[14];
    state[10] = tmp1;
    state[14] = tmp2;

    // Row 3: shift right by 3 (= shift left by 1)
    let tmp = state[3];
    state[3] = state[7];
    state[7] = state[11];
    state[11] = state[15];
    state[15] = tmp;
}

/// MixColumns transformation
fn mix_columns(state: &mut [u8; 16]) {
    for col in 0..4 {
        let i = col * 4;
        let s0 = state[i];
        let s1 = state[i + 1];
        let s2 = state[i + 2];
        let s3 = state[i + 3];

        state[i] = gf_mul2(s0) ^ gf_mul3(s1) ^ s2 ^ s3;
        state[i + 1] = s0 ^ gf_mul2(s1) ^ gf_mul3(s2) ^ s3;
        state[i + 2] = s0 ^ s1 ^ gf_mul2(s2) ^ gf_mul3(s3);
        state[i + 3] = gf_mul3(s0) ^ s1 ^ s2 ^ gf_mul2(s3);
    }
}

/// Inverse MixColumns transformation
fn inv_mix_columns(state: &mut [u8; 16]) {
    for col in 0..4 {
        let i = col * 4;
        let s0 = state[i];
        let s1 = state[i + 1];
        let s2 = state[i + 2];
        let s3 = state[i + 3];

        state[i] = gf_mul14(s0) ^ gf_mul11(s1) ^ gf_mul13(s2) ^ gf_mul9(s3);
        state[i + 1] = gf_mul9(s0) ^ gf_mul14(s1) ^ gf_mul11(s2) ^ gf_mul13(s3);
        state[i + 2] = gf_mul13(s0) ^ gf_mul9(s1) ^ gf_mul14(s2) ^ gf_mul11(s3);
        state[i + 3] = gf_mul11(s0) ^ gf_mul13(s1) ^ gf_mul9(s2) ^ gf_mul14(s3);
    }
}

/// AddRoundKey transformation
fn add_round_key(state: &mut [u8; 16], round_key: &[u8; 16]) {
    for (s, k) in state.iter_mut().zip(round_key.iter()) {
        *s ^= k;
    }
}

/// Expand 128-bit key to 11 round keys
pub fn expand_key_128(key: &[u8; 16]) -> [[u8; 16]; 11] {
    let mut w = [[0u8; 16]; 11];
    w[0] = *key;

    for i in 1..11 {
        // Copy previous round key to avoid borrow issues
        let prev = w[i - 1];
        let mut temp = [prev[12], prev[13], prev[14], prev[15]];

        // RotWord
        temp.rotate_left(1);
        // SubWord
        for byte in &mut temp {
            *byte = SBOX[*byte as usize];
        }
        // XOR with Rcon
        temp[0] ^= RCON[i];

        // First word XOR with temp
        for k in 0..4 {
            w[i][k] = prev[k] ^ temp[k];
        }
        // Remaining words
        for j in 1..4 {
            for k in 0..4 {
                w[i][j * 4 + k] = prev[j * 4 + k] ^ w[i][(j - 1) * 4 + k];
            }
        }
    }

    w
}

/// Expand 256-bit key to 15 round keys
pub fn expand_key_256(key: &[u8; 32]) -> [[u8; 16]; 15] {
    let mut w = [[0u8; 16]; 15];

    // First two round keys are the original key
    w[0].copy_from_slice(&key[0..16]);
    w[1].copy_from_slice(&key[16..32]);

    for i in 2..15 {
        // Copy previous round keys to avoid borrow issues
        let prev = w[i - 1];
        let prev2 = w[i - 2];

        if i % 2 == 0 {
            // Every other round key
            let mut temp = [prev[12], prev[13], prev[14], prev[15]];
            temp.rotate_left(1);
            for byte in &mut temp {
                *byte = SBOX[*byte as usize];
            }
            temp[0] ^= RCON[i / 2];

            // First 4 bytes XOR with temp
            for j in 0..4 {
                w[i][j] = prev2[j] ^ temp[j];
            }
            // Remaining bytes XOR with previous byte in current round key
            for j in 4..16 {
                w[i][j] = prev2[j] ^ w[i][j - 4];
            }
        } else {
            // Alternate round keys (SubWord only, no RotWord or Rcon)
            let mut temp = [prev[12], prev[13], prev[14], prev[15]];
            for byte in &mut temp {
                *byte = SBOX[*byte as usize];
            }

            // First 4 bytes XOR with temp
            for j in 0..4 {
                w[i][j] = prev2[j] ^ temp[j];
            }
            // Remaining bytes XOR with previous byte in current round key
            for j in 4..16 {
                w[i][j] = prev2[j] ^ w[i][j - 4];
            }
        }
    }

    w
}

/// Encrypt a single 16-byte block with AES-256
pub fn aes_encrypt_block(block: &[u8; 16], round_keys: &[[u8; 16]; 15]) -> [u8; 16] {
    let mut state = *block;

    // Initial round key addition
    add_round_key(&mut state, &round_keys[0]);

    // Main rounds (1-13)
    for round in 1..14 {
        sub_bytes(&mut state);
        shift_rows(&mut state);
        mix_columns(&mut state);
        add_round_key(&mut state, &round_keys[round]);
    }

    // Final round (no MixColumns)
    sub_bytes(&mut state);
    shift_rows(&mut state);
    add_round_key(&mut state, &round_keys[14]);

    state
}

/// Decrypt a single 16-byte block with AES-256
pub fn aes_decrypt_block(block: &[u8; 16], round_keys: &[[u8; 16]; 15]) -> [u8; 16] {
    let mut state = *block;

    // Initial round key addition (reverse order)
    add_round_key(&mut state, &round_keys[14]);

    // Main rounds (13-1)
    for round in (1..14).rev() {
        inv_shift_rows(&mut state);
        inv_sub_bytes(&mut state);
        add_round_key(&mut state, &round_keys[round]);
        inv_mix_columns(&mut state);
    }

    // Final round (no InvMixColumns)
    inv_shift_rows(&mut state);
    inv_sub_bytes(&mut state);
    add_round_key(&mut state, &round_keys[0]);

    state
}

/// Encrypt a single 16-byte block with AES-128
pub fn aes128_encrypt_block(block: &[u8; 16], round_keys: &[[u8; 16]; 11]) -> [u8; 16] {
    let mut state = *block;

    add_round_key(&mut state, &round_keys[0]);

    for round in 1..10 {
        sub_bytes(&mut state);
        shift_rows(&mut state);
        mix_columns(&mut state);
        add_round_key(&mut state, &round_keys[round]);
    }

    sub_bytes(&mut state);
    shift_rows(&mut state);
    add_round_key(&mut state, &round_keys[10]);

    state
}

/// Decrypt a single 16-byte block with AES-128
pub fn aes128_decrypt_block(block: &[u8; 16], round_keys: &[[u8; 16]; 11]) -> [u8; 16] {
    let mut state = *block;

    add_round_key(&mut state, &round_keys[10]);

    for round in (1..10).rev() {
        inv_shift_rows(&mut state);
        inv_sub_bytes(&mut state);
        add_round_key(&mut state, &round_keys[round]);
        inv_mix_columns(&mut state);
    }

    inv_shift_rows(&mut state);
    inv_sub_bytes(&mut state);
    add_round_key(&mut state, &round_keys[0]);

    state
}

/// AES-CBC encrypt
pub fn aes_cbc_encrypt(plaintext: &[u8], key: &[u8; 32], iv: &[u8; 16]) -> alloc::vec::Vec<u8> {
    let round_keys = expand_key_256(key);
    let mut ciphertext = alloc::vec::Vec::with_capacity(plaintext.len());
    let mut prev_block = *iv;

    let blocks = plaintext.len() / 16;
    for i in 0..blocks {
        let mut block = [0u8; 16];
        block.copy_from_slice(&plaintext[i * 16..(i + 1) * 16]);

        // XOR with previous ciphertext block
        for j in 0..16 {
            block[j] ^= prev_block[j];
        }

        let encrypted = aes_encrypt_block(&block, &round_keys);
        ciphertext.extend_from_slice(&encrypted);
        prev_block = encrypted;
    }

    ciphertext
}

/// AES-CBC decrypt
pub fn aes_cbc_decrypt(ciphertext: &[u8], key: &[u8; 32], iv: &[u8; 16]) -> alloc::vec::Vec<u8> {
    let round_keys = expand_key_256(key);
    let mut plaintext = alloc::vec::Vec::with_capacity(ciphertext.len());
    let mut prev_block = *iv;

    let blocks = ciphertext.len() / 16;
    for i in 0..blocks {
        let mut block = [0u8; 16];
        block.copy_from_slice(&ciphertext[i * 16..(i + 1) * 16]);

        let decrypted = aes_decrypt_block(&block, &round_keys);

        // XOR with previous ciphertext block
        let mut result = [0u8; 16];
        for j in 0..16 {
            result[j] = decrypted[j] ^ prev_block[j];
        }

        plaintext.extend_from_slice(&result);
        prev_block = block;
    }

    plaintext
}

/// AES-CTR encrypt/decrypt (symmetric)
pub fn aes_ctr(data: &[u8], key: &[u8; 32], nonce: &[u8; 16]) -> alloc::vec::Vec<u8> {
    let round_keys = expand_key_256(key);
    let mut output = alloc::vec::Vec::with_capacity(data.len());
    let mut counter = *nonce;

    let blocks = (data.len() + 15) / 16;
    for i in 0..blocks {
        let keystream = aes_encrypt_block(&counter, &round_keys);

        let start = i * 16;
        let end = core::cmp::min(start + 16, data.len());

        for j in start..end {
            output.push(data[j] ^ keystream[j - start]);
        }

        // Increment counter
        for j in (0..16).rev() {
            counter[j] = counter[j].wrapping_add(1);
            if counter[j] != 0 {
                break;
            }
        }
    }

    output
}
