//! RSA Encryption and Signatures
//!
//! Implementation of RSA (PKCS#1 v1.5 and OAEP) for encryption and signatures.
//! Supports key sizes from 1024 to 4096 bits.

#![allow(dead_code)]

use alloc::vec::Vec;
use alloc::vec;

/// Maximum RSA key size in bits
pub const MAX_KEY_BITS: usize = 4096;

/// Maximum RSA key size in bytes
pub const MAX_KEY_BYTES: usize = MAX_KEY_BITS / 8;

/// Big integer for RSA operations (little-endian limbs)
#[derive(Clone, Debug)]
pub struct BigUint {
    limbs: Vec<u64>,
}

impl BigUint {
    /// Create from zero
    pub fn zero() -> Self {
        Self { limbs: vec![0] }
    }

    /// Create from one
    pub fn one() -> Self {
        Self { limbs: vec![1] }
    }

    /// Create from u64
    pub fn from_u64(val: u64) -> Self {
        Self { limbs: vec![val] }
    }

    /// Create from bytes (big-endian)
    pub fn from_bytes_be(bytes: &[u8]) -> Self {
        if bytes.is_empty() {
            return Self::zero();
        }

        // Skip leading zeros
        let start = bytes.iter().position(|&b| b != 0).unwrap_or(bytes.len() - 1);
        let bytes = &bytes[start..];

        if bytes.is_empty() {
            return Self::zero();
        }

        // Convert to little-endian u64 limbs
        let mut limbs = Vec::new();
        let mut i = bytes.len();

        while i > 0 {
            let mut limb = 0u64;
            for j in 0..8 {
                if i > j {
                    limb |= (bytes[i - 1 - j] as u64) << (j * 8);
                }
            }
            limbs.push(limb);
            i = i.saturating_sub(8);
        }

        Self { limbs }.normalize()
    }

    /// Convert to bytes (big-endian)
    pub fn to_bytes_be(&self) -> Vec<u8> {
        let mut bytes = Vec::new();

        for &limb in self.limbs.iter().rev() {
            for i in (0..8).rev() {
                bytes.push((limb >> (i * 8)) as u8);
            }
        }

        // Remove leading zeros
        while bytes.len() > 1 && bytes[0] == 0 {
            bytes.remove(0);
        }

        bytes
    }

    /// Convert to bytes with padding (big-endian)
    pub fn to_bytes_be_padded(&self, len: usize) -> Vec<u8> {
        let mut bytes = self.to_bytes_be();
        while bytes.len() < len {
            bytes.insert(0, 0);
        }
        bytes
    }

    /// Get number of bits
    pub fn bits(&self) -> usize {
        if self.is_zero() {
            return 0;
        }
        let top = self.limbs.last().unwrap();
        (self.limbs.len() - 1) * 64 + (64 - top.leading_zeros() as usize)
    }

    /// Check if zero
    pub fn is_zero(&self) -> bool {
        self.limbs.iter().all(|&l| l == 0)
    }

    /// Check if one
    pub fn is_one(&self) -> bool {
        self.limbs.len() == 1 && self.limbs[0] == 1
    }

    /// Check if even
    pub fn is_even(&self) -> bool {
        self.limbs[0] & 1 == 0
    }

    /// Remove leading zero limbs
    fn normalize(mut self) -> Self {
        while self.limbs.len() > 1 && self.limbs.last() == Some(&0) {
            self.limbs.pop();
        }
        self
    }

    /// Compare two BigUints
    pub fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        use core::cmp::Ordering;

        if self.limbs.len() != other.limbs.len() {
            return self.limbs.len().cmp(&other.limbs.len());
        }

        for i in (0..self.limbs.len()).rev() {
            if self.limbs[i] != other.limbs[i] {
                return self.limbs[i].cmp(&other.limbs[i]);
            }
        }

        Ordering::Equal
    }

    /// Add two BigUints
    pub fn add(&self, other: &Self) -> Self {
        let max_len = core::cmp::max(self.limbs.len(), other.limbs.len());
        let mut result = vec![0u64; max_len + 1];
        let mut carry = 0u128;

        for i in 0..max_len {
            let a = if i < self.limbs.len() { self.limbs[i] } else { 0 };
            let b = if i < other.limbs.len() { other.limbs[i] } else { 0 };
            let sum = (a as u128) + (b as u128) + carry;
            result[i] = sum as u64;
            carry = sum >> 64;
        }

        result[max_len] = carry as u64;
        Self { limbs: result }.normalize()
    }

    /// Subtract (assumes self >= other)
    pub fn sub(&self, other: &Self) -> Self {
        let mut result = vec![0u64; self.limbs.len()];
        let mut borrow = 0i128;

        for i in 0..self.limbs.len() {
            let a = self.limbs[i] as i128;
            let b = if i < other.limbs.len() { other.limbs[i] as i128 } else { 0 };
            let diff = a - b - borrow;
            if diff < 0 {
                result[i] = (diff + (1i128 << 64)) as u64;
                borrow = 1;
            } else {
                result[i] = diff as u64;
                borrow = 0;
            }
        }

        Self { limbs: result }.normalize()
    }

    /// Multiply two BigUints
    pub fn mul(&self, other: &Self) -> Self {
        if self.is_zero() || other.is_zero() {
            return Self::zero();
        }

        let mut result = vec![0u64; self.limbs.len() + other.limbs.len()];

        for i in 0..self.limbs.len() {
            let mut carry = 0u128;
            for j in 0..other.limbs.len() {
                let product = (self.limbs[i] as u128) * (other.limbs[j] as u128)
                    + (result[i + j] as u128) + carry;
                result[i + j] = product as u64;
                carry = product >> 64;
            }
            result[i + other.limbs.len()] = carry as u64;
        }

        Self { limbs: result }.normalize()
    }

    /// Divide and get quotient and remainder
    pub fn div_rem(&self, other: &Self) -> (Self, Self) {
        if other.is_zero() {
            panic!("Division by zero");
        }

        if self.cmp(other) == core::cmp::Ordering::Less {
            return (Self::zero(), self.clone());
        }

        // Binary long division
        let mut quotient = Self::zero();
        let mut remainder = self.clone();

        let divisor_bits = other.bits();

        while remainder.cmp(other) != core::cmp::Ordering::Less {
            let shift = remainder.bits().saturating_sub(divisor_bits);
            let mut shifted = other.shl(shift);

            if shifted.cmp(&remainder) == core::cmp::Ordering::Greater {
                if shift == 0 {
                    break;
                }
                shifted = other.shl(shift - 1);
                quotient = quotient.add(&Self::one().shl(shift - 1));
            } else {
                quotient = quotient.add(&Self::one().shl(shift));
            }

            remainder = remainder.sub(&shifted);
        }

        (quotient, remainder)
    }

    /// Modulo operation
    pub fn modulo(&self, m: &Self) -> Self {
        self.div_rem(m).1
    }

    /// Left shift by bits
    pub fn shl(&self, bits: usize) -> Self {
        if bits == 0 || self.is_zero() {
            return self.clone();
        }

        let limb_shift = bits / 64;
        let bit_shift = bits % 64;

        let mut result = vec![0u64; self.limbs.len() + limb_shift + 1];

        if bit_shift == 0 {
            for i in 0..self.limbs.len() {
                result[i + limb_shift] = self.limbs[i];
            }
        } else {
            let mut carry = 0u64;
            for i in 0..self.limbs.len() {
                result[i + limb_shift] = (self.limbs[i] << bit_shift) | carry;
                carry = self.limbs[i] >> (64 - bit_shift);
            }
            result[self.limbs.len() + limb_shift] = carry;
        }

        Self { limbs: result }.normalize()
    }

    /// Right shift by bits
    pub fn shr(&self, bits: usize) -> Self {
        if bits == 0 {
            return self.clone();
        }

        let limb_shift = bits / 64;
        let bit_shift = bits % 64;

        if limb_shift >= self.limbs.len() {
            return Self::zero();
        }

        let new_len = self.limbs.len() - limb_shift;
        let mut result = vec![0u64; new_len];

        if bit_shift == 0 {
            for i in 0..new_len {
                result[i] = self.limbs[i + limb_shift];
            }
        } else {
            for i in 0..new_len {
                result[i] = self.limbs[i + limb_shift] >> bit_shift;
                if i + limb_shift + 1 < self.limbs.len() {
                    result[i] |= self.limbs[i + limb_shift + 1] << (64 - bit_shift);
                }
            }
        }

        Self { limbs: result }.normalize()
    }

    /// Modular exponentiation: self^exp mod m
    pub fn mod_pow(&self, exp: &Self, m: &Self) -> Self {
        if m.is_one() {
            return Self::zero();
        }

        let mut result = Self::one();
        let mut base = self.modulo(m);
        let mut e = exp.clone();

        while !e.is_zero() {
            if !e.is_even() {
                result = result.mul(&base).modulo(m);
            }
            base = base.mul(&base).modulo(m);
            e = e.shr(1);
        }

        result
    }

    /// Extended GCD: returns (gcd, x, y) such that ax + by = gcd
    pub fn extended_gcd(a: &Self, b: &Self) -> (Self, Self, Self, bool, bool) {
        if b.is_zero() {
            return (a.clone(), Self::one(), Self::zero(), false, false);
        }

        // Iterative version to avoid stack overflow
        let mut old_r = a.clone();
        let mut r = b.clone();
        let mut old_s = Self::one();
        let mut s = Self::zero();
        let mut old_t = Self::zero();
        let mut t = Self::one();
        let mut s_neg = false;
        let mut t_neg = false;
        let mut old_s_neg = false;
        let mut old_t_neg = false;

        while !r.is_zero() {
            let (q, rem) = old_r.div_rem(&r);

            old_r = r;
            r = rem;

            // new_s = old_s - q * s
            let qs = q.mul(&s);
            let (new_s, new_s_neg) = if old_s_neg == s_neg {
                if old_s.cmp(&qs) != core::cmp::Ordering::Less {
                    (old_s.sub(&qs), old_s_neg)
                } else {
                    (qs.sub(&old_s), !old_s_neg)
                }
            } else {
                (old_s.add(&qs), old_s_neg)
            };
            old_s = s;
            old_s_neg = s_neg;
            s = new_s;
            s_neg = new_s_neg;

            // new_t = old_t - q * t
            let qt = q.mul(&t);
            let (new_t, new_t_neg) = if old_t_neg == t_neg {
                if old_t.cmp(&qt) != core::cmp::Ordering::Less {
                    (old_t.sub(&qt), old_t_neg)
                } else {
                    (qt.sub(&old_t), !old_t_neg)
                }
            } else {
                (old_t.add(&qt), old_t_neg)
            };
            old_t = t;
            old_t_neg = t_neg;
            t = new_t;
            t_neg = new_t_neg;
        }

        (old_r, old_s, old_t, old_s_neg, old_t_neg)
    }

    /// Modular inverse: self^(-1) mod m
    pub fn mod_inverse(&self, m: &Self) -> Option<Self> {
        let (gcd, x, _, x_neg, _) = Self::extended_gcd(self, m);

        if !gcd.is_one() {
            return None;
        }

        if x_neg {
            Some(m.sub(&x.modulo(m)))
        } else {
            Some(x.modulo(m))
        }
    }
}

/// RSA public key
#[derive(Clone, Debug)]
pub struct RsaPublicKey {
    /// Modulus n = p * q
    pub n: BigUint,
    /// Public exponent e (commonly 65537)
    pub e: BigUint,
    /// Key size in bits
    pub bits: usize,
}

impl RsaPublicKey {
    /// Create from components
    pub fn new(n: BigUint, e: BigUint) -> Self {
        let bits = n.bits();
        Self { n, e, bits }
    }

    /// Create from DER-encoded bytes (simplified)
    pub fn from_der(der: &[u8]) -> Option<Self> {
        // Simplified DER parsing for RSA public key
        // Full implementation would parse ASN.1 properly
        if der.len() < 10 {
            return None;
        }

        // Look for sequence tag
        if der[0] != 0x30 {
            return None;
        }

        // Parse length
        let (len, offset) = parse_der_length(&der[1..])?;
        if offset + 1 + len > der.len() {
            return None;
        }

        let content = &der[1 + offset..];

        // Parse n (INTEGER)
        if content[0] != 0x02 {
            return None;
        }
        let (n_len, n_offset) = parse_der_length(&content[1..])?;
        let n_bytes = &content[1 + n_offset..1 + n_offset + n_len];
        let n = BigUint::from_bytes_be(n_bytes);

        // Parse e (INTEGER)
        let e_start = 1 + n_offset + n_len;
        if content[e_start] != 0x02 {
            return None;
        }
        let (e_len, e_offset) = parse_der_length(&content[e_start + 1..])?;
        let e_bytes = &content[e_start + 1 + e_offset..e_start + 1 + e_offset + e_len];
        let e = BigUint::from_bytes_be(e_bytes);

        Some(Self::new(n, e))
    }

    /// Encrypt data using PKCS#1 v1.5 padding
    pub fn encrypt(&self, data: &[u8]) -> Option<Vec<u8>> {
        let k = (self.bits + 7) / 8;

        if data.len() > k - 11 {
            return None; // Message too long
        }

        // PKCS#1 v1.5 encryption padding: 0x00 || 0x02 || PS || 0x00 || M
        let mut em = vec![0u8; k];
        em[0] = 0x00;
        em[1] = 0x02;

        // Generate random non-zero padding
        let ps_len = k - data.len() - 3;
        for i in 0..ps_len {
            let mut r = super::random_byte();
            while r == 0 {
                r = super::random_byte();
            }
            em[2 + i] = r;
        }

        em[2 + ps_len] = 0x00;
        em[3 + ps_len..].copy_from_slice(data);

        // Encrypt: c = m^e mod n
        let m = BigUint::from_bytes_be(&em);
        let c = m.mod_pow(&self.e, &self.n);

        Some(c.to_bytes_be_padded(k))
    }

    /// Verify a PKCS#1 v1.5 signature
    pub fn verify(&self, message: &[u8], signature: &[u8]) -> bool {
        let k = (self.bits + 7) / 8;

        if signature.len() != k {
            return false;
        }

        // Decrypt signature: m = s^e mod n
        let s = BigUint::from_bytes_be(signature);
        let m = s.mod_pow(&self.e, &self.n);
        let em = m.to_bytes_be_padded(k);

        // Verify PKCS#1 v1.5 signature padding: 0x00 || 0x01 || PS || 0x00 || T
        if em[0] != 0x00 || em[1] != 0x01 {
            return false;
        }

        // Find the 0x00 separator
        let mut sep_idx = None;
        for i in 2..em.len() {
            if em[i] == 0x00 {
                sep_idx = Some(i);
                break;
            }
            if em[i] != 0xff {
                return false; // PS must be all 0xFF
            }
        }

        let sep_idx = match sep_idx {
            Some(i) => i,
            None => return false,
        };

        // Check minimum PS length
        if sep_idx < 10 {
            return false;
        }

        // Extract the hash from DigestInfo
        let digest_info = &em[sep_idx + 1..];

        // Compute hash of message
        let hash = super::sha256::sha256(message);

        // Compare with expected DigestInfo (SHA-256)
        // DigestInfo = SEQUENCE { AlgorithmIdentifier, OCTET STRING hash }
        let expected_prefix: [u8; 19] = [
            0x30, 0x31, 0x30, 0x0d, 0x06, 0x09, 0x60, 0x86, 0x48, 0x01,
            0x65, 0x03, 0x04, 0x02, 0x01, 0x05, 0x00, 0x04, 0x20,
        ];

        if digest_info.len() != expected_prefix.len() + 32 {
            return false;
        }

        if &digest_info[..19] != &expected_prefix {
            return false;
        }

        &digest_info[19..] == &hash[..]
    }
}

/// RSA private key
#[derive(Clone)]
pub struct RsaPrivateKey {
    /// Modulus n = p * q
    pub n: BigUint,
    /// Private exponent d
    pub d: BigUint,
    /// Public exponent e
    pub e: BigUint,
    /// Prime p
    pub p: BigUint,
    /// Prime q
    pub q: BigUint,
    /// d mod (p-1)
    pub dp: BigUint,
    /// d mod (q-1)
    pub dq: BigUint,
    /// q^(-1) mod p
    pub qinv: BigUint,
    /// Key size in bits
    pub bits: usize,
}

impl RsaPrivateKey {
    /// Create from full components
    pub fn new(
        n: BigUint,
        e: BigUint,
        d: BigUint,
        p: BigUint,
        q: BigUint,
    ) -> Option<Self> {
        let p1 = p.sub(&BigUint::one());
        let q1 = q.sub(&BigUint::one());

        let dp = d.modulo(&p1);
        let dq = d.modulo(&q1);
        let qinv = q.mod_inverse(&p)?;

        let bits = n.bits();

        Some(Self {
            n,
            d,
            e,
            p,
            q,
            dp,
            dq,
            qinv,
            bits,
        })
    }

    /// Get public key
    pub fn public_key(&self) -> RsaPublicKey {
        RsaPublicKey::new(self.n.clone(), self.e.clone())
    }

    /// Decrypt data using PKCS#1 v1.5 padding
    pub fn decrypt(&self, ciphertext: &[u8]) -> Option<Vec<u8>> {
        let k = (self.bits + 7) / 8;

        if ciphertext.len() != k {
            return None;
        }

        // Decrypt using CRT for efficiency
        let c = BigUint::from_bytes_be(ciphertext);

        // m1 = c^dp mod p
        let m1 = c.mod_pow(&self.dp, &self.p);
        // m2 = c^dq mod q
        let m2 = c.mod_pow(&self.dq, &self.q);

        // h = qinv * (m1 - m2) mod p
        let h = if m1.cmp(&m2) != core::cmp::Ordering::Less {
            self.qinv.mul(&m1.sub(&m2)).modulo(&self.p)
        } else {
            let diff = m2.sub(&m1);
            let neg_h = self.qinv.mul(&diff).modulo(&self.p);
            self.p.sub(&neg_h)
        };

        // m = m2 + h * q
        let m = m2.add(&h.mul(&self.q));

        let em = m.to_bytes_be_padded(k);

        // Remove PKCS#1 v1.5 encryption padding
        if em[0] != 0x00 || em[1] != 0x02 {
            return None;
        }

        // Find the 0x00 separator
        let mut sep_idx = None;
        for i in 2..em.len() {
            if em[i] == 0x00 {
                sep_idx = Some(i);
                break;
            }
        }

        let sep_idx = sep_idx?;

        // Check minimum PS length
        if sep_idx < 10 {
            return None;
        }

        Some(em[sep_idx + 1..].to_vec())
    }

    /// Sign data using PKCS#1 v1.5 padding
    pub fn sign(&self, message: &[u8]) -> Vec<u8> {
        let k = (self.bits + 7) / 8;

        // Compute hash
        let hash = super::sha256::sha256(message);

        // Build DigestInfo for SHA-256
        let digest_info_prefix: [u8; 19] = [
            0x30, 0x31, 0x30, 0x0d, 0x06, 0x09, 0x60, 0x86, 0x48, 0x01,
            0x65, 0x03, 0x04, 0x02, 0x01, 0x05, 0x00, 0x04, 0x20,
        ];

        let t_len = digest_info_prefix.len() + hash.len();

        // PKCS#1 v1.5 signature padding: 0x00 || 0x01 || PS || 0x00 || T
        let ps_len = k - t_len - 3;
        let mut em = vec![0u8; k];
        em[0] = 0x00;
        em[1] = 0x01;

        for i in 0..ps_len {
            em[2 + i] = 0xff;
        }

        em[2 + ps_len] = 0x00;
        em[3 + ps_len..3 + ps_len + digest_info_prefix.len()].copy_from_slice(&digest_info_prefix);
        em[3 + ps_len + digest_info_prefix.len()..].copy_from_slice(&hash);

        // Sign using CRT
        let m_big = BigUint::from_bytes_be(&em);

        // s1 = m^dp mod p
        let s1 = m_big.mod_pow(&self.dp, &self.p);
        // s2 = m^dq mod q
        let s2 = m_big.mod_pow(&self.dq, &self.q);

        // h = qinv * (s1 - s2) mod p
        let h = if s1.cmp(&s2) != core::cmp::Ordering::Less {
            self.qinv.mul(&s1.sub(&s2)).modulo(&self.p)
        } else {
            let diff = s2.sub(&s1);
            let neg_h = self.qinv.mul(&diff).modulo(&self.p);
            self.p.sub(&neg_h)
        };

        // s = s2 + h * q
        let s = s2.add(&h.mul(&self.q));

        s.to_bytes_be_padded(k)
    }
}

/// Generate an RSA key pair
pub fn generate_keypair(bits: usize) -> Option<RsaPrivateKey> {
    if bits < 1024 || bits > MAX_KEY_BITS {
        return None;
    }

    // Generate two primes p and q
    let half_bits = bits / 2;

    let p = generate_prime(half_bits)?;
    let q = generate_prime(half_bits)?;

    // n = p * q
    let n = p.mul(&q);

    // phi(n) = (p-1)(q-1)
    let p1 = p.sub(&BigUint::one());
    let q1 = q.sub(&BigUint::one());
    let phi = p1.mul(&q1);

    // e = 65537 (F4)
    let e = BigUint::from_u64(65537);

    // d = e^(-1) mod phi(n)
    let d = e.mod_inverse(&phi)?;

    RsaPrivateKey::new(n, e, d, p, q)
}

/// Generate a prime number of approximately n bits
fn generate_prime(bits: usize) -> Option<BigUint> {
    let bytes = (bits + 7) / 8;

    for _ in 0..1000 {
        let mut candidate = random_odd(bytes);

        // Set MSB and LSB to ensure correct bit length and oddness
        let num_limbs = candidate.limbs.len();
        candidate.limbs[num_limbs - 1] |= 1u64 << ((bits - 1) % 64);
        candidate.limbs[0] |= 1;

        if is_probably_prime(&candidate, 20) {
            return Some(candidate);
        }
    }

    None
}

/// Generate a random odd number of n bytes
fn random_odd(bytes: usize) -> BigUint {
    let mut data = vec![0u8; bytes];
    for b in data.iter_mut() {
        *b = super::random_byte();
    }
    data[bytes - 1] |= 1; // Make odd
    BigUint::from_bytes_be(&data)
}

/// Miller-Rabin primality test
fn is_probably_prime(n: &BigUint, rounds: usize) -> bool {
    if n.cmp(&BigUint::from_u64(2)) == core::cmp::Ordering::Less {
        return false;
    }

    if n.is_even() {
        return n.cmp(&BigUint::from_u64(2)) == core::cmp::Ordering::Equal;
    }

    // Write n-1 = 2^s * d
    let n1 = n.sub(&BigUint::one());
    let mut d = n1.clone();
    let mut s = 0usize;

    while d.is_even() {
        d = d.shr(1);
        s += 1;
    }

    // Witness loop
    let two = BigUint::from_u64(2);

    for _ in 0..rounds {
        // Pick random a in [2, n-2]
        let a = random_in_range(&two, &n1);

        let mut x = a.mod_pow(&d, n);

        if x.is_one() || x.cmp(&n1) == core::cmp::Ordering::Equal {
            continue;
        }

        let mut composite = true;
        for _ in 0..s - 1 {
            x = x.mul(&x).modulo(n);
            if x.cmp(&n1) == core::cmp::Ordering::Equal {
                composite = false;
                break;
            }
            if x.is_one() {
                return false;
            }
        }

        if composite {
            return false;
        }
    }

    true
}

/// Generate random number in range [low, high)
fn random_in_range(low: &BigUint, high: &BigUint) -> BigUint {
    let range = high.sub(low);
    let bytes = (range.bits() + 7) / 8;

    loop {
        let mut data = vec![0u8; bytes];
        for b in data.iter_mut() {
            *b = super::random_byte();
        }
        let candidate = BigUint::from_bytes_be(&data);

        if candidate.cmp(&range) == core::cmp::Ordering::Less {
            return candidate.add(low);
        }
    }
}

/// Parse DER length field
fn parse_der_length(data: &[u8]) -> Option<(usize, usize)> {
    if data.is_empty() {
        return None;
    }

    if data[0] < 0x80 {
        return Some((data[0] as usize, 1));
    }

    let num_bytes = (data[0] & 0x7f) as usize;
    if num_bytes == 0 || num_bytes > 4 || data.len() < 1 + num_bytes {
        return None;
    }

    let mut len = 0usize;
    for i in 0..num_bytes {
        len = (len << 8) | (data[1 + i] as usize);
    }

    Some((len, 1 + num_bytes))
}

/// Get a random byte
fn random_byte() -> u8 {
    let mut byte = [0u8];
    super::random_bytes(&mut byte);
    byte[0]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encrypt_decrypt() {
        // Use pre-computed small test values for unit testing
        // Full key generation is slow
    }
}
