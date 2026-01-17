//! WiFi Cryptography
//!
//! Cryptographic functions for WiFi security (WEP, TKIP, CCMP).

use alloc::vec::Vec;
use crate::util::{KResult, KError};

/// Key types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyType {
    /// Pairwise Transient Key
    Ptk,
    /// Group Temporal Key
    Gtk,
    /// Pairwise Master Key
    Pmk,
    /// Master Session Key
    Msk,
}

/// Cipher key
#[derive(Clone)]
pub struct CipherKey {
    /// Key type
    pub key_type: KeyType,
    /// Cipher type
    pub cipher: CipherType,
    /// Key data
    pub key: Vec<u8>,
    /// Key index (for GTK)
    pub key_index: u8,
    /// Transmit key flag
    pub tx_key: bool,
    /// Receive sequence counter
    pub rx_seq: u64,
    /// Transmit sequence counter
    pub tx_seq: u64,
}

impl CipherKey {
    /// Create new key
    pub fn new(key_type: KeyType, cipher: CipherType, key: Vec<u8>) -> Self {
        Self {
            key_type,
            cipher,
            key,
            key_index: 0,
            tx_key: false,
            rx_seq: 0,
            tx_seq: 0,
        }
    }

    /// Get key length for cipher
    pub fn key_length(&self) -> usize {
        self.cipher.key_length()
    }

    /// Increment TX sequence counter
    pub fn next_tx_seq(&mut self) -> u64 {
        let seq = self.tx_seq;
        self.tx_seq = self.tx_seq.wrapping_add(1);
        seq
    }
}

/// Cipher types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CipherType {
    /// No encryption
    None,
    /// WEP-40 (64-bit)
    Wep40,
    /// WEP-104 (128-bit)
    Wep104,
    /// TKIP (WPA)
    Tkip,
    /// CCMP (WPA2 - AES-CCM)
    Ccmp,
    /// GCMP (WPA3 - AES-GCM)
    Gcmp,
}

impl CipherType {
    /// Get key length in bytes
    pub fn key_length(self) -> usize {
        match self {
            CipherType::None => 0,
            CipherType::Wep40 => 5,
            CipherType::Wep104 => 13,
            CipherType::Tkip => 32, // 16 TK + 8 MIC TX + 8 MIC RX
            CipherType::Ccmp => 16,
            CipherType::Gcmp => 16,
        }
    }

    /// Get IV length in bytes
    pub fn iv_length(self) -> usize {
        match self {
            CipherType::None => 0,
            CipherType::Wep40 | CipherType::Wep104 => 4,
            CipherType::Tkip => 8,
            CipherType::Ccmp => 8,
            CipherType::Gcmp => 8,
        }
    }

    /// Get MIC length in bytes
    pub fn mic_length(self) -> usize {
        match self {
            CipherType::None | CipherType::Wep40 | CipherType::Wep104 => 0,
            CipherType::Tkip => 8,
            CipherType::Ccmp => 8,
            CipherType::Gcmp => 16,
        }
    }

    /// Get header length (IV + extended IV)
    pub fn header_length(self) -> usize {
        match self {
            CipherType::None => 0,
            CipherType::Wep40 | CipherType::Wep104 => 4,
            CipherType::Tkip | CipherType::Ccmp | CipherType::Gcmp => 8,
        }
    }

    /// Get trailer length (ICV/MIC)
    pub fn trailer_length(self) -> usize {
        match self {
            CipherType::None => 0,
            CipherType::Wep40 | CipherType::Wep104 => 4, // ICV
            CipherType::Tkip => 12, // ICV + MIC
            CipherType::Ccmp => 8,  // MIC
            CipherType::Gcmp => 16, // Tag
        }
    }
}

/// CCMP (AES-CCM) encryption/decryption
pub struct Ccmp {
    key: [u8; 16],
}

impl Ccmp {
    /// Create new CCMP instance
    pub fn new(key: &[u8]) -> KResult<Self> {
        if key.len() != 16 {
            return Err(KError::Invalid);
        }
        let mut k = [0u8; 16];
        k.copy_from_slice(key);
        Ok(Self { key: k })
    }

    /// Encrypt data with CCMP
    pub fn encrypt(
        &self,
        header: &[u8],
        data: &[u8],
        nonce: &[u8; 13],
    ) -> KResult<Vec<u8>> {
        // AES-CCM encryption
        // In a full implementation, this would use AES-CCM mode

        let mut output = Vec::with_capacity(data.len() + 8); // data + MIC

        // For now, implement a placeholder that maintains the structure
        // Real implementation would need AES cipher

        // Generate CCMP header (IV)
        let pn = nonce_to_pn(nonce);
        let ccmp_header = build_ccmp_header(pn, 0);
        output.extend_from_slice(&ccmp_header);

        // Encrypt data (placeholder - XOR with derived key for structure)
        let keystream = derive_keystream(&self.key, nonce, data.len());
        for (i, &byte) in data.iter().enumerate() {
            output.push(byte ^ keystream[i]);
        }

        // Generate MIC (placeholder)
        let mic = compute_ccm_mic(&self.key, nonce, header, data);
        output.extend_from_slice(&mic);

        Ok(output)
    }

    /// Decrypt data with CCMP
    pub fn decrypt(
        &self,
        header: &[u8],
        data: &[u8],
    ) -> KResult<Vec<u8>> {
        if data.len() < 16 { // 8 header + 8 MIC minimum
            return Err(KError::Invalid);
        }

        // Extract CCMP header
        let ccmp_header = &data[..8];
        let encrypted = &data[8..data.len() - 8];
        let mic = &data[data.len() - 8..];

        // Extract PN from header
        let pn = extract_pn(ccmp_header);
        let nonce = pn_to_nonce(pn, &header[10..16]); // addr2

        // Decrypt (placeholder)
        let keystream = derive_keystream(&self.key, &nonce, encrypted.len());
        let mut plaintext = Vec::with_capacity(encrypted.len());
        for (i, &byte) in encrypted.iter().enumerate() {
            plaintext.push(byte ^ keystream[i]);
        }

        // Verify MIC (placeholder)
        let expected_mic = compute_ccm_mic(&self.key, &nonce, header, &plaintext);
        if mic != expected_mic.as_slice() {
            return Err(KError::Invalid);
        }

        Ok(plaintext)
    }
}

/// Build CCMP header from PN
fn build_ccmp_header(pn: u64, key_id: u8) -> [u8; 8] {
    [
        (pn & 0xFF) as u8,
        ((pn >> 8) & 0xFF) as u8,
        0, // Reserved
        0x20 | ((key_id & 0x03) << 6), // Ext IV flag + Key ID
        ((pn >> 16) & 0xFF) as u8,
        ((pn >> 24) & 0xFF) as u8,
        ((pn >> 32) & 0xFF) as u8,
        ((pn >> 40) & 0xFF) as u8,
    ]
}

/// Extract PN from CCMP header
fn extract_pn(header: &[u8]) -> u64 {
    if header.len() < 8 {
        return 0;
    }
    (header[0] as u64) |
    ((header[1] as u64) << 8) |
    ((header[4] as u64) << 16) |
    ((header[5] as u64) << 24) |
    ((header[6] as u64) << 32) |
    ((header[7] as u64) << 40)
}

/// Convert nonce to PN
fn nonce_to_pn(nonce: &[u8; 13]) -> u64 {
    (nonce[7] as u64) |
    ((nonce[8] as u64) << 8) |
    ((nonce[9] as u64) << 16) |
    ((nonce[10] as u64) << 24) |
    ((nonce[11] as u64) << 32) |
    ((nonce[12] as u64) << 40)
}

/// Convert PN to nonce
fn pn_to_nonce(pn: u64, addr: &[u8]) -> [u8; 13] {
    let mut nonce = [0u8; 13];
    nonce[0] = 0; // Priority
    if addr.len() >= 6 {
        nonce[1..7].copy_from_slice(&addr[..6]);
    }
    nonce[7] = (pn & 0xFF) as u8;
    nonce[8] = ((pn >> 8) & 0xFF) as u8;
    nonce[9] = ((pn >> 16) & 0xFF) as u8;
    nonce[10] = ((pn >> 24) & 0xFF) as u8;
    nonce[11] = ((pn >> 32) & 0xFF) as u8;
    nonce[12] = ((pn >> 40) & 0xFF) as u8;
    nonce
}

/// Derive keystream (placeholder - needs real AES)
fn derive_keystream(key: &[u8; 16], nonce: &[u8; 13], len: usize) -> Vec<u8> {
    // This is a placeholder - real implementation needs AES-CTR
    let mut stream = Vec::with_capacity(len);
    let mut state = [0u8; 16];

    // Initialize state from key and nonce
    for i in 0..16 {
        state[i] = key[i] ^ nonce[i % 13];
    }

    // Generate keystream
    let mut counter = 0u32;
    while stream.len() < len {
        // Mix in counter
        let counter_bytes = counter.to_le_bytes();
        for i in 0..4 {
            state[12 + i] ^= counter_bytes[i];
        }

        // Simple mixing (not real AES!)
        for i in 0..16 {
            state[i] = state[i].wrapping_add(state[(i + 1) % 16])
                .rotate_left((i % 8) as u32);
        }

        stream.extend_from_slice(&state);
        counter += 1;
    }

    stream.truncate(len);
    stream
}

/// Compute CCM MIC (placeholder - needs real AES-CBC-MAC)
fn compute_ccm_mic(key: &[u8; 16], nonce: &[u8; 13], aad: &[u8], data: &[u8]) -> [u8; 8] {
    // This is a placeholder - real implementation needs AES-CBC-MAC
    let mut mic = [0u8; 8];

    // Simple hash combining key, nonce, AAD, and data
    let mut hash: u32 = 0x811c9dc5;
    for &b in key {
        hash ^= b as u32;
        hash = hash.wrapping_mul(0x01000193);
    }
    for &b in nonce {
        hash ^= b as u32;
        hash = hash.wrapping_mul(0x01000193);
    }
    for &b in aad {
        hash ^= b as u32;
        hash = hash.wrapping_mul(0x01000193);
    }
    for &b in data {
        hash ^= b as u32;
        hash = hash.wrapping_mul(0x01000193);
    }

    // Fill MIC
    for i in 0..8 {
        mic[i] = ((hash >> ((i % 4) * 8)) & 0xFF) as u8;
        hash = hash.rotate_left(3);
    }

    mic
}

/// TKIP encryption/decryption (legacy WPA)
pub struct Tkip {
    tk: [u8; 16],
    mic_tx_key: [u8; 8],
    mic_rx_key: [u8; 8],
}

impl Tkip {
    /// Create new TKIP instance
    pub fn new(key: &[u8]) -> KResult<Self> {
        if key.len() != 32 {
            return Err(KError::Invalid);
        }

        let mut tk = [0u8; 16];
        let mut mic_tx = [0u8; 8];
        let mut mic_rx = [0u8; 8];

        tk.copy_from_slice(&key[..16]);
        mic_tx.copy_from_slice(&key[16..24]);
        mic_rx.copy_from_slice(&key[24..32]);

        Ok(Self {
            tk,
            mic_tx_key: mic_tx,
            mic_rx_key: mic_rx,
        })
    }

    /// Compute Michael MIC
    pub fn michael_mic(&self, header: &[u8], data: &[u8], tx: bool) -> [u8; 8] {
        let key = if tx { &self.mic_tx_key } else { &self.mic_rx_key };
        michael(key, header, data)
    }
}

/// Michael MIC algorithm
fn michael(key: &[u8; 8], header: &[u8], data: &[u8]) -> [u8; 8] {
    let mut l = u32::from_le_bytes([key[0], key[1], key[2], key[3]]);
    let mut r = u32::from_le_bytes([key[4], key[5], key[6], key[7]]);

    // Process header (DA + SA + priority)
    let mut block = [0u8; 4];
    let all_data = [header, data].concat();
    let padded_len = (all_data.len() + 5 + 3) & !3; // +5 for padding, round up
    let mut padded = all_data.clone();
    padded.push(0x5a); // Padding start
    while padded.len() < padded_len {
        padded.push(0);
    }

    for chunk in padded.chunks(4) {
        if chunk.len() == 4 {
            block.copy_from_slice(chunk);
            let word = u32::from_le_bytes(block);
            (l, r) = michael_block(l, r, word);
        }
    }

    let mut result = [0u8; 8];
    result[..4].copy_from_slice(&l.to_le_bytes());
    result[4..].copy_from_slice(&r.to_le_bytes());
    result
}

/// Michael block function
fn michael_block(l: u32, r: u32, word: u32) -> (u32, u32) {
    let mut l = l ^ word;
    let mut r = r;

    r ^= l.rotate_left(17);
    l = l.wrapping_add(r);
    r ^= ((l & 0xff00ff00) >> 8) | ((l & 0x00ff00ff) << 8);
    l = l.wrapping_add(r);
    r ^= l.rotate_left(3);
    l = l.wrapping_add(r);
    r ^= l.rotate_right(2);
    l = l.wrapping_add(r);

    (l, r)
}

/// WEP encryption (legacy, insecure)
pub struct Wep {
    key: Vec<u8>,
}

impl Wep {
    /// Create new WEP instance
    pub fn new(key: &[u8]) -> KResult<Self> {
        if key.len() != 5 && key.len() != 13 {
            return Err(KError::Invalid);
        }
        Ok(Self { key: key.to_vec() })
    }

    /// Encrypt with WEP
    pub fn encrypt(&self, data: &[u8], iv: [u8; 3], key_id: u8) -> Vec<u8> {
        // RC4 encryption
        let mut full_key = Vec::with_capacity(3 + self.key.len());
        full_key.extend_from_slice(&iv);
        full_key.extend_from_slice(&self.key);

        let keystream = rc4_keystream(&full_key, data.len() + 4);

        let mut output = Vec::with_capacity(4 + data.len() + 4);

        // IV + Key ID
        output.extend_from_slice(&iv);
        output.push(key_id << 6);

        // Encrypted data
        for (i, &byte) in data.iter().enumerate() {
            output.push(byte ^ keystream[i]);
        }

        // ICV (CRC32 of plaintext, encrypted)
        let icv = crc32(data);
        let icv_bytes = icv.to_le_bytes();
        for (i, &byte) in icv_bytes.iter().enumerate() {
            output.push(byte ^ keystream[data.len() + i]);
        }

        output
    }

    /// Decrypt with WEP
    pub fn decrypt(&self, data: &[u8]) -> KResult<Vec<u8>> {
        if data.len() < 8 { // 4 IV + 4 ICV minimum
            return Err(KError::Invalid);
        }

        let iv = [data[0], data[1], data[2]];
        let encrypted = &data[4..data.len() - 4];
        let encrypted_icv = &data[data.len() - 4..];

        let mut full_key = Vec::with_capacity(3 + self.key.len());
        full_key.extend_from_slice(&iv);
        full_key.extend_from_slice(&self.key);

        let keystream = rc4_keystream(&full_key, encrypted.len() + 4);

        let mut plaintext = Vec::with_capacity(encrypted.len());
        for (i, &byte) in encrypted.iter().enumerate() {
            plaintext.push(byte ^ keystream[i]);
        }

        // Decrypt and verify ICV
        let mut icv_bytes = [0u8; 4];
        for i in 0..4 {
            icv_bytes[i] = encrypted_icv[i] ^ keystream[encrypted.len() + i];
        }
        let received_icv = u32::from_le_bytes(icv_bytes);
        let computed_icv = crc32(&plaintext);

        if received_icv != computed_icv {
            return Err(KError::Invalid);
        }

        Ok(plaintext)
    }
}

/// RC4 keystream generation
fn rc4_keystream(key: &[u8], len: usize) -> Vec<u8> {
    let mut s = [0u8; 256];
    for i in 0..256 {
        s[i] = i as u8;
    }

    // KSA
    let mut j: u8 = 0;
    for i in 0..256 {
        j = j.wrapping_add(s[i]).wrapping_add(key[i % key.len()]);
        s.swap(i, j as usize);
    }

    // PRGA
    let mut output = Vec::with_capacity(len);
    let mut i: u8 = 0;
    let mut j: u8 = 0;

    for _ in 0..len {
        i = i.wrapping_add(1);
        j = j.wrapping_add(s[i as usize]);
        s.swap(i as usize, j as usize);
        let k = s[s[i as usize].wrapping_add(s[j as usize]) as usize];
        output.push(k);
    }

    output
}

/// CRC32 calculation
fn crc32(data: &[u8]) -> u32 {
    const CRC32_TABLE: [u32; 256] = generate_crc32_table();

    let mut crc = 0xFFFFFFFF_u32;
    for &byte in data {
        let index = ((crc ^ byte as u32) & 0xFF) as usize;
        crc = (crc >> 8) ^ CRC32_TABLE[index];
    }
    !crc
}

/// Generate CRC32 lookup table
const fn generate_crc32_table() -> [u32; 256] {
    let mut table = [0u32; 256];
    let mut i = 0;
    while i < 256 {
        let mut crc = i as u32;
        let mut j = 0;
        while j < 8 {
            if crc & 1 != 0 {
                crc = (crc >> 1) ^ 0xEDB88320;
            } else {
                crc >>= 1;
            }
            j += 1;
        }
        table[i] = crc;
        i += 1;
    }
    table
}

/// Key derivation function for WPA-PSK (PBKDF2-SHA1)
pub fn derive_pmk(passphrase: &str, ssid: &str) -> [u8; 32] {
    // PBKDF2-SHA1 with 4096 iterations
    pbkdf2_sha1(passphrase.as_bytes(), ssid.as_bytes(), 4096, 32)
        .try_into()
        .unwrap_or([0u8; 32])
}

/// PBKDF2-SHA1 key derivation
fn pbkdf2_sha1(password: &[u8], salt: &[u8], iterations: u32, dk_len: usize) -> Vec<u8> {
    let mut dk = Vec::with_capacity(dk_len);
    let mut block = 1u32;

    while dk.len() < dk_len {
        let mut u = hmac_sha1(password, &[salt, &block.to_be_bytes()].concat());
        let mut f = u.clone();

        for _ in 1..iterations {
            u = hmac_sha1(password, &u);
            for (i, &byte) in u.iter().enumerate() {
                f[i] ^= byte;
            }
        }

        dk.extend_from_slice(&f);
        block += 1;
    }

    dk.truncate(dk_len);
    dk
}

/// HMAC-SHA1
pub fn hmac_sha1(key: &[u8], message: &[u8]) -> [u8; 20] {
    const BLOCK_SIZE: usize = 64;
    const IPAD: u8 = 0x36;
    const OPAD: u8 = 0x5c;

    // Prepare key
    let mut k = [0u8; BLOCK_SIZE];
    if key.len() > BLOCK_SIZE {
        let h = sha1(key);
        k[..20].copy_from_slice(&h);
    } else {
        k[..key.len()].copy_from_slice(key);
    }

    // Inner hash
    let mut inner = Vec::with_capacity(BLOCK_SIZE + message.len());
    for &b in &k {
        inner.push(b ^ IPAD);
    }
    inner.extend_from_slice(message);
    let inner_hash = sha1(&inner);

    // Outer hash
    let mut outer = Vec::with_capacity(BLOCK_SIZE + 20);
    for &b in &k {
        outer.push(b ^ OPAD);
    }
    outer.extend_from_slice(&inner_hash);
    sha1(&outer)
}

/// SHA1 hash (simplified implementation)
fn sha1(data: &[u8]) -> [u8; 20] {
    let mut h: [u32; 5] = [
        0x67452301, 0xEFCDAB89, 0x98BADCFE, 0x10325476, 0xC3D2E1F0,
    ];

    // Pad message
    let ml = (data.len() as u64) * 8;
    let mut padded = data.to_vec();
    padded.push(0x80);
    while (padded.len() % 64) != 56 {
        padded.push(0);
    }
    padded.extend_from_slice(&ml.to_be_bytes());

    // Process blocks
    for chunk in padded.chunks(64) {
        let mut w = [0u32; 80];

        // Initialize first 16 words
        for (i, word) in chunk.chunks(4).enumerate() {
            w[i] = u32::from_be_bytes([word[0], word[1], word[2], word[3]]);
        }

        // Extend to 80 words
        for i in 16..80 {
            w[i] = (w[i - 3] ^ w[i - 8] ^ w[i - 14] ^ w[i - 16]).rotate_left(1);
        }

        let mut a = h[0];
        let mut b = h[1];
        let mut c = h[2];
        let mut d = h[3];
        let mut e = h[4];

        for i in 0..80 {
            let (f, k) = match i {
                0..=19 => ((b & c) | ((!b) & d), 0x5A827999_u32),
                20..=39 => (b ^ c ^ d, 0x6ED9EBA1_u32),
                40..=59 => ((b & c) | (b & d) | (c & d), 0x8F1BBCDC_u32),
                _ => (b ^ c ^ d, 0xCA62C1D6_u32),
            };

            let temp = a.rotate_left(5)
                .wrapping_add(f)
                .wrapping_add(e)
                .wrapping_add(k)
                .wrapping_add(w[i]);
            e = d;
            d = c;
            c = b.rotate_left(30);
            b = a;
            a = temp;
        }

        h[0] = h[0].wrapping_add(a);
        h[1] = h[1].wrapping_add(b);
        h[2] = h[2].wrapping_add(c);
        h[3] = h[3].wrapping_add(d);
        h[4] = h[4].wrapping_add(e);
    }

    let mut result = [0u8; 20];
    for (i, &word) in h.iter().enumerate() {
        result[i * 4..(i + 1) * 4].copy_from_slice(&word.to_be_bytes());
    }
    result
}

/// PRF (Pseudo-Random Function) for WPA key expansion
pub fn prf(key: &[u8], label: &str, data: &[u8], output_len: usize) -> Vec<u8> {
    let mut result = Vec::with_capacity(output_len);
    let mut counter = 0u8;

    while result.len() < output_len {
        let mut input = Vec::new();
        input.extend_from_slice(label.as_bytes());
        input.push(0);
        input.extend_from_slice(data);
        input.push(counter);

        let hash = hmac_sha1(key, &input);
        result.extend_from_slice(&hash);
        counter += 1;
    }

    result.truncate(output_len);
    result
}
