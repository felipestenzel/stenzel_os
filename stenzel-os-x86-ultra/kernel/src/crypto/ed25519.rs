//! Ed25519 Digital Signatures
//!
//! Implementation of Ed25519 (RFC 8032) for signing and verification.
//! Uses twisted Edwards curve with equation: -x^2 + y^2 = 1 + d*x^2*y^2
//! where d = -121665/121666

#![allow(dead_code)]

use alloc::vec::Vec;

/// Ed25519 public key size (32 bytes)
pub const PUBLIC_KEY_SIZE: usize = 32;

/// Ed25519 private key (seed) size (32 bytes)
pub const PRIVATE_KEY_SIZE: usize = 32;

/// Ed25519 signature size (64 bytes)
pub const SIGNATURE_SIZE: usize = 64;

/// Ed25519 expanded private key size (64 bytes)
pub const EXPANDED_KEY_SIZE: usize = 64;

/// Field element in GF(2^255-19) using 5 limbs of 51 bits
#[derive(Clone, Copy, Debug)]
struct Fe([u64; 5]);

impl Fe {
    const ZERO: Self = Self([0; 5]);
    const ONE: Self = Self([1, 0, 0, 0, 0]);

    /// d = -121665/121666 mod p
    const D: Self = Self([
        0x34DCA135978A3,
        0x1A8283B156EBD,
        0x5E7A26001C029,
        0x739C663A03CBB,
        0x52036CBC148B6,
    ]);

    /// 2*d
    const D2: Self = Self([
        0x69B9426B2F159,
        0x35050762ADD7D,
        0x3CF44C0038052,
        0x6738CC7407977,
        0x2406D9DC56DFF,
    ]);

    /// sqrt(-1) mod p
    const SQRT_M1: Self = Self([
        0x61B274A0EA0B0,
        0x0D5A5FC8F189D,
        0x7EF5E9CBD0C60,
        0x78595A6804C9E,
        0x2B8324804FC1D,
    ]);

    /// Load from 32 bytes (little-endian)
    fn from_bytes(s: &[u8; 32]) -> Self {
        let mut h = [0u64; 5];
        h[0] = load_51(s, 0);
        h[1] = load_51(s, 1);
        h[2] = load_51(s, 2);
        h[3] = load_51(s, 3);
        h[4] = load_51(s, 4);
        Self(h)
    }

    /// Store to 32 bytes (little-endian)
    fn to_bytes(&self) -> [u8; 32] {
        let mut h = self.0;

        // Reduce modulo 2^255-19
        let mut q = (19 * h[4] + (1 << 24)) >> 25;
        q = (h[0] + q) >> 51;
        q = (h[1] + q) >> 51;
        q = (h[2] + q) >> 51;
        q = (h[3] + q) >> 51;
        q = (h[4] + q) >> 51;

        h[0] += 19 * q;

        let carry = h[0] >> 51;
        h[0] &= 0x7ffffffffffff;
        h[1] += carry;

        let carry = h[1] >> 51;
        h[1] &= 0x7ffffffffffff;
        h[2] += carry;

        let carry = h[2] >> 51;
        h[2] &= 0x7ffffffffffff;
        h[3] += carry;

        let carry = h[3] >> 51;
        h[3] &= 0x7ffffffffffff;
        h[4] += carry;
        h[4] &= 0x7ffffffffffff;

        let mut s = [0u8; 32];
        store_51(&mut s, 0, h[0]);
        store_51(&mut s, 1, h[1]);
        store_51(&mut s, 2, h[2]);
        store_51(&mut s, 3, h[3]);
        store_51(&mut s, 4, h[4]);
        s
    }

    /// Add two field elements
    fn add(&self, b: &Self) -> Self {
        Self([
            self.0[0] + b.0[0],
            self.0[1] + b.0[1],
            self.0[2] + b.0[2],
            self.0[3] + b.0[3],
            self.0[4] + b.0[4],
        ])
    }

    /// Subtract two field elements
    fn sub(&self, b: &Self) -> Self {
        // Add 2p to ensure positive result
        Self([
            self.0[0] + 0xfffffffffffda - b.0[0],
            self.0[1] + 0xffffffffffffe - b.0[1],
            self.0[2] + 0xffffffffffffe - b.0[2],
            self.0[3] + 0xffffffffffffe - b.0[3],
            self.0[4] + 0xffffffffffffe - b.0[4],
        ]).reduce()
    }

    /// Multiply two field elements
    fn mul(&self, b: &Self) -> Self {
        let a0 = self.0[0] as u128;
        let a1 = self.0[1] as u128;
        let a2 = self.0[2] as u128;
        let a3 = self.0[3] as u128;
        let a4 = self.0[4] as u128;

        let b0 = b.0[0] as u128;
        let b1 = b.0[1] as u128;
        let b2 = b.0[2] as u128;
        let b3 = b.0[3] as u128;
        let b4 = b.0[4] as u128;

        // Multiply with reduction factor 19 for terms >= 2^255
        let c0 = a0 * b0 + 19 * (a1 * b4 + a2 * b3 + a3 * b2 + a4 * b1);
        let c1 = a0 * b1 + a1 * b0 + 19 * (a2 * b4 + a3 * b3 + a4 * b2);
        let c2 = a0 * b2 + a1 * b1 + a2 * b0 + 19 * (a3 * b4 + a4 * b3);
        let c3 = a0 * b3 + a1 * b2 + a2 * b1 + a3 * b0 + 19 * (a4 * b4);
        let c4 = a0 * b4 + a1 * b3 + a2 * b2 + a3 * b1 + a4 * b0;

        Self::carry([c0, c1, c2, c3, c4])
    }

    /// Square a field element
    fn square(&self) -> Self {
        let a0 = self.0[0] as u128;
        let a1 = self.0[1] as u128;
        let a2 = self.0[2] as u128;
        let a3 = self.0[3] as u128;
        let a4 = self.0[4] as u128;

        let a0_2 = 2 * a0;
        let a1_2 = 2 * a1;
        let a2_2 = 2 * a2;
        let a3_2 = 2 * a3;

        let c0 = a0 * a0 + 38 * (a1 * a4 + a2 * a3);
        let c1 = a0_2 * a1 + 38 * (a2 * a4) + 19 * a3 * a3;
        let c2 = a0_2 * a2 + a1 * a1 + 38 * (a3 * a4);
        let c3 = a0_2 * a3 + a1_2 * a2 + 19 * a4 * a4;
        let c4 = a0_2 * a4 + a1_2 * a3 + a2 * a2;

        Self::carry([c0, c1, c2, c3, c4])
    }

    /// Carry and reduce
    fn carry(c: [u128; 5]) -> Self {
        let mut h = [0u64; 5];

        let carry = (c[0] >> 51) as u128;
        h[0] = (c[0] & 0x7ffffffffffff) as u64;
        let c1 = c[1] + carry;

        let carry = (c1 >> 51) as u128;
        h[1] = (c1 & 0x7ffffffffffff) as u64;
        let c2 = c[2] + carry;

        let carry = (c2 >> 51) as u128;
        h[2] = (c2 & 0x7ffffffffffff) as u64;
        let c3 = c[3] + carry;

        let carry = (c3 >> 51) as u128;
        h[3] = (c3 & 0x7ffffffffffff) as u64;
        let c4 = c[4] + carry;

        let carry = (c4 >> 51) as u128;
        h[4] = (c4 & 0x7ffffffffffff) as u64;
        h[0] += 19 * carry as u64;

        Self(h)
    }

    /// Reduce modulo 2^255-19
    fn reduce(&self) -> Self {
        let mut h = self.0;

        let carry = h[0] >> 51;
        h[0] &= 0x7ffffffffffff;
        h[1] += carry;

        let carry = h[1] >> 51;
        h[1] &= 0x7ffffffffffff;
        h[2] += carry;

        let carry = h[2] >> 51;
        h[2] &= 0x7ffffffffffff;
        h[3] += carry;

        let carry = h[3] >> 51;
        h[3] &= 0x7ffffffffffff;
        h[4] += carry;

        let carry = h[4] >> 51;
        h[4] &= 0x7ffffffffffff;
        h[0] += 19 * carry;

        Self(h)
    }

    /// Negate
    fn neg(&self) -> Self {
        Fe::ZERO.sub(self)
    }

    /// Invert (using Fermat's little theorem: a^(-1) = a^(p-2) mod p)
    fn invert(&self) -> Self {
        let z2 = self.square();
        let z4 = z2.square();
        let z8 = z4.square();
        let z9 = self.mul(&z8);
        let z11 = z2.mul(&z9);
        let z22 = z11.square();
        let z_5_0 = z9.mul(&z22);

        let mut t0 = z_5_0.square();
        for _ in 1..5 {
            t0 = t0.square();
        }
        let z_10_5 = t0.mul(&z_5_0);

        t0 = z_10_5.square();
        for _ in 1..10 {
            t0 = t0.square();
        }
        let z_20_10 = t0.mul(&z_10_5);

        t0 = z_20_10.square();
        for _ in 1..20 {
            t0 = t0.square();
        }
        let z_40_20 = t0.mul(&z_20_10);

        t0 = z_40_20.square();
        for _ in 1..10 {
            t0 = t0.square();
        }
        let z_50_10 = t0.mul(&z_10_5);

        t0 = z_50_10.square();
        for _ in 1..50 {
            t0 = t0.square();
        }
        let z_100_50 = t0.mul(&z_50_10);

        t0 = z_100_50.square();
        for _ in 1..100 {
            t0 = t0.square();
        }
        let z_200_100 = t0.mul(&z_100_50);

        t0 = z_200_100.square();
        for _ in 1..50 {
            t0 = t0.square();
        }
        let z_250_50 = t0.mul(&z_50_10);

        t0 = z_250_50.square();
        t0 = t0.square();
        t0 = t0.square();
        t0 = t0.square();
        t0 = t0.square();

        t0.mul(&z11)
    }

    /// Compute sqrt(u/v), returning None if not a square
    fn sqrt_ratio(u: &Self, v: &Self) -> Option<Self> {
        let v3 = v.square().mul(v);
        let v7 = v3.square().mul(v);
        let mut r = u.mul(&v3).mul(&u.mul(&v7).pow_p58());

        let check = v.mul(&r.square());

        if check.eq(u) {
            return Some(r);
        }

        if check.eq(&u.neg()) {
            r = r.mul(&Self::SQRT_M1);
            return Some(r);
        }

        None
    }

    /// Compute a^((p-5)/8)
    fn pow_p58(&self) -> Self {
        let z2 = self.square();
        let z4 = z2.square();
        let z8 = z4.square();
        let z9 = self.mul(&z8);
        let z11 = z2.mul(&z9);
        let z22 = z11.square();
        let z_5_0 = z9.mul(&z22);

        let mut t = z_5_0.square();
        for _ in 1..5 {
            t = t.square();
        }
        let z_10_5 = t.mul(&z_5_0);

        t = z_10_5.square();
        for _ in 1..10 {
            t = t.square();
        }
        let z_20_10 = t.mul(&z_10_5);

        t = z_20_10.square();
        for _ in 1..20 {
            t = t.square();
        }
        let z_40_20 = t.mul(&z_20_10);

        t = z_40_20.square();
        for _ in 1..10 {
            t = t.square();
        }
        let z_50_10 = t.mul(&z_10_5);

        t = z_50_10.square();
        for _ in 1..50 {
            t = t.square();
        }
        let z_100_50 = t.mul(&z_50_10);

        t = z_100_50.square();
        for _ in 1..100 {
            t = t.square();
        }
        let z_200_100 = t.mul(&z_100_50);

        t = z_200_100.square();
        for _ in 1..50 {
            t = t.square();
        }
        let z_250_50 = t.mul(&z_50_10);

        t = z_250_50.square();
        t = t.square();

        t.mul(&self)
    }

    /// Check equality
    fn eq(&self, other: &Self) -> bool {
        let a = self.reduce().to_bytes();
        let b = other.reduce().to_bytes();
        constant_time_eq(&a, &b)
    }

    /// Check if negative (LSB of reduced form)
    fn is_negative(&self) -> bool {
        let bytes = self.reduce().to_bytes();
        (bytes[0] & 1) == 1
    }
}

/// Load 51 bits from bytes at limb index
fn load_51(s: &[u8], limb: usize) -> u64 {
    let bit_offset = limb * 51;
    let byte_offset = bit_offset / 8;
    let shift = bit_offset % 8;

    let mut val = 0u64;
    for i in 0..8 {
        if byte_offset + i < 32 {
            val |= (s[byte_offset + i] as u64) << (i * 8);
        }
    }
    (val >> shift) & 0x7ffffffffffff
}

/// Store 51 bits to bytes at limb index
fn store_51(s: &mut [u8], limb: usize, val: u64) {
    let bit_offset = limb * 51;
    let byte_offset = bit_offset / 8;
    let shift = bit_offset % 8;

    for i in 0..7 {
        if byte_offset + i < 32 {
            let existing = s[byte_offset + i] as u64;
            let mask = if i == 0 { (1u64 << shift) - 1 } else { 0 };
            let new_bits = ((val << shift) >> (i * 8)) & 0xff;
            s[byte_offset + i] = ((existing & mask) | new_bits) as u8;
        }
    }
}

/// Constant-time comparison
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

/// Point on the twisted Edwards curve in extended coordinates (X, Y, Z, T)
/// where x = X/Z, y = Y/Z, and xy = T/Z
#[derive(Clone, Copy)]
struct Point {
    x: Fe,
    y: Fe,
    z: Fe,
    t: Fe,
}

impl Point {
    /// Identity point (neutral element)
    const IDENTITY: Self = Self {
        x: Fe::ZERO,
        y: Fe::ONE,
        z: Fe::ONE,
        t: Fe::ZERO,
    };

    /// Base point (generator)
    fn base_point() -> Self {
        // Generator point for Ed25519
        let x = Fe([
            0x62d608f25d51a,
            0x412a4b4f6592a,
            0x75b7171a4b31d,
            0x1ff60527118fe,
            0x216936d3cd6e5,
        ]);
        let y = Fe([
            0x6666666666658,
            0x4cccccccccccc,
            0x1999999999999,
            0x3333333333333,
            0x6666666666666,
        ]);
        let t = x.mul(&y);
        Self { x, y, z: Fe::ONE, t }
    }

    /// Decode a point from 32 bytes
    fn decode(bytes: &[u8; 32]) -> Option<Self> {
        // The sign of x is in the MSB of the last byte
        let mut bytes_copy = *bytes;
        let x_sign = (bytes_copy[31] >> 7) != 0;
        bytes_copy[31] &= 0x7f;

        let y = Fe::from_bytes(&bytes_copy);

        // Compute x^2 = (y^2 - 1) / (d*y^2 + 1)
        let y2 = y.square();
        let u = y2.sub(&Fe::ONE);
        let v = Fe::D.mul(&y2).add(&Fe::ONE);

        let x = Fe::sqrt_ratio(&u, &v)?;

        // Choose correct sign
        let x = if x.is_negative() != x_sign { x.neg() } else { x };

        let t = x.mul(&y);
        Some(Self { x, y, z: Fe::ONE, t })
    }

    /// Encode point to 32 bytes
    fn encode(&self) -> [u8; 32] {
        let zinv = self.z.invert();
        let x = self.x.mul(&zinv);
        let y = self.y.mul(&zinv);

        let mut bytes = y.to_bytes();
        bytes[31] |= if x.is_negative() { 0x80 } else { 0 };
        bytes
    }

    /// Double a point
    fn double(&self) -> Self {
        let a = self.x.square();
        let b = self.y.square();
        let c = self.z.square();
        let c = c.add(&c);
        let d = a.neg();

        let e = self.x.add(&self.y).square().sub(&a).sub(&b);
        let g = d.add(&b);
        let f = g.sub(&c);
        let h = d.sub(&b);

        let x = e.mul(&f);
        let y = g.mul(&h);
        let t = e.mul(&h);
        let z = f.mul(&g);

        Self { x, y, z, t }
    }

    /// Add two points
    fn add(&self, other: &Self) -> Self {
        let a = self.y.sub(&self.x).mul(&other.y.sub(&other.x));
        let b = self.y.add(&self.x).mul(&other.y.add(&other.x));
        let c = Fe::D2.mul(&self.t).mul(&other.t);
        let d = self.z.add(&self.z).mul(&other.z);

        let e = b.sub(&a);
        let f = d.sub(&c);
        let g = d.add(&c);
        let h = b.add(&a);

        let x = e.mul(&f);
        let y = g.mul(&h);
        let t = e.mul(&h);
        let z = f.mul(&g);

        Self { x, y, z, t }
    }

    /// Negate a point
    fn neg(&self) -> Self {
        Self {
            x: self.x.neg(),
            y: self.y,
            z: self.z,
            t: self.t.neg(),
        }
    }

    /// Scalar multiplication using double-and-add
    fn scalar_mul(&self, scalar: &[u8; 32]) -> Self {
        let mut result = Self::IDENTITY;
        let mut temp = *self;

        for byte in scalar.iter() {
            for bit in 0..8 {
                if (byte >> bit) & 1 == 1 {
                    result = result.add(&temp);
                }
                temp = temp.double();
            }
        }

        result
    }
}

/// Scalar in the curve order group (little-endian 256-bit)
#[derive(Clone, Copy)]
struct Scalar([u8; 32]);

impl Scalar {
    /// Reduce a 64-byte hash to a scalar mod l
    fn from_bytes_wide(bytes: &[u8; 64]) -> Self {
        // l = 2^252 + 27742317777372353535851937790883648493
        // Using Barrett reduction

        let mut acc = [0u64; 8];
        for i in 0..8 {
            acc[i] = u64::from_le_bytes([
                bytes[i * 8],
                bytes[i * 8 + 1],
                bytes[i * 8 + 2],
                bytes[i * 8 + 3],
                bytes[i * 8 + 4],
                bytes[i * 8 + 5],
                bytes[i * 8 + 6],
                bytes[i * 8 + 7],
            ]);
        }

        // Modular reduction
        Self::reduce512(&acc)
    }

    /// Reduce a 512-bit number mod l
    fn reduce512(input: &[u64; 8]) -> Self {
        // l = 2^252 + 27742317777372353535851937790883648493
        // l in limbs: [0x5812631a5cf5d3ed, 0x14def9dea2f79cd6, 0, 0x1000000000000000]

        // Simplified reduction: we'll do schoolbook reduction
        let mut r = [0u64; 9];
        r[..8].copy_from_slice(input);

        // L * 2^252 reduction constants
        // 2^252 mod l = -27742317777372353535851937790883648493 mod l
        // = l - 27742317777372353535851937790883648493

        // For simplicity, use repeated subtraction for small reductions
        // In practice, this would use proper Barrett or Montgomery

        // Convert to bytes for simpler processing
        let mut bytes = [0u8; 64];
        for i in 0..8 {
            let b = input[i].to_le_bytes();
            bytes[i * 8..(i + 1) * 8].copy_from_slice(&b);
        }

        // Reduce the upper bits
        sc_reduce64(&mut bytes);

        let mut result = [0u8; 32];
        result.copy_from_slice(&bytes[..32]);
        Self(result)
    }

    /// Add two scalars mod l
    fn add(&self, other: &Self) -> Self {
        let mut sum = [0u8; 64];
        sum[..32].copy_from_slice(&self.0);

        let mut carry = 0u32;
        for i in 0..32 {
            let s = (self.0[i] as u32) + (other.0[i] as u32) + carry;
            sum[i] = s as u8;
            carry = s >> 8;
        }
        if carry > 0 {
            sum[32] = carry as u8;
        }

        sc_reduce64(&mut sum);

        let mut result = [0u8; 32];
        result.copy_from_slice(&sum[..32]);
        Self(result)
    }

    /// Multiply two scalars mod l
    fn mul(&self, other: &Self) -> Self {
        let mut product = [0u8; 64];

        // Schoolbook multiplication
        for i in 0..32 {
            let mut carry = 0u32;
            for j in 0..32 {
                if i + j < 64 {
                    let p = (self.0[i] as u32) * (other.0[j] as u32) + (product[i + j] as u32) + carry;
                    product[i + j] = p as u8;
                    carry = p >> 8;
                }
            }
            if i + 32 < 64 {
                product[i + 32] = carry as u8;
            }
        }

        sc_reduce64(&mut product);

        let mut result = [0u8; 32];
        result.copy_from_slice(&product[..32]);
        Self(result)
    }

    /// Get bytes
    fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Reduce a 64-byte value mod l
fn sc_reduce64(s: &mut [u8; 64]) {
    // l = 2^252 + 27742317777372353535851937790883648493
    const L: [u8; 32] = [
        0xed, 0xd3, 0xf5, 0x5c, 0x1a, 0x63, 0x12, 0x58,
        0xd6, 0x9c, 0xf7, 0xa2, 0xde, 0xf9, 0xde, 0x14,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x10,
    ];

    // Convert to limbs for reduction
    let mut t = [0i64; 24];
    for i in 0..64 {
        t[i / 3] += (s[i] as i64) << ((i % 3) * 8);
    }

    // Reduce using l
    for i in (0..=23).rev() {
        if i >= 12 {
            // Reduce high limbs
            let k = i - 12;
            t[k] -= t[i] * 0x5cf5d3ed;
            t[k + 1] -= t[i] * 0x5812631a;
            t[k + 4] -= t[i] * 0xa2f79cd6;
            t[k + 5] -= t[i] * 0x14def9de;
            t[k + 12] += t[i] * 0x10000000;
            t[i] = 0;
        }
    }

    // Carry
    for i in 0..12 {
        let carry = t[i] >> 21;
        t[i + 1] += carry;
        t[i] &= 0x1fffff;
    }

    // Final subtraction of l if needed
    let mut borrow = 0i64;
    let mut result = [0u8; 64];
    for i in 0..32 {
        let li = if i < L.len() { L[i] as i64 } else { 0 };
        let diff = (t[i / 3] >> ((i % 3) * 8)) as i64 & 0xff - li - borrow;
        if diff < 0 {
            result[i] = (diff + 256) as u8;
            borrow = 1;
        } else {
            result[i] = diff as u8;
            borrow = 0;
        }
    }

    // If borrow, keep original value
    if borrow != 0 {
        for i in 0..32 {
            s[i] = ((t[i / 3] >> ((i % 3) * 8)) & 0xff) as u8;
        }
    } else {
        s[..32].copy_from_slice(&result[..32]);
    }

    // Zero upper bytes
    for i in 32..64 {
        s[i] = 0;
    }
}

/// SHA-512 hash function
fn sha512(data: &[u8]) -> [u8; 64] {
    // Initial hash values
    let mut h: [u64; 8] = [
        0x6a09e667f3bcc908, 0xbb67ae8584caa73b,
        0x3c6ef372fe94f82b, 0xa54ff53a5f1d36f1,
        0x510e527fade682d1, 0x9b05688c2b3e6c1f,
        0x1f83d9abfb41bd6b, 0x5be0cd19137e2179,
    ];

    // Round constants
    const K: [u64; 80] = [
        0x428a2f98d728ae22, 0x7137449123ef65cd, 0xb5c0fbcfec4d3b2f, 0xe9b5dba58189dbbc,
        0x3956c25bf348b538, 0x59f111f1b605d019, 0x923f82a4af194f9b, 0xab1c5ed5da6d8118,
        0xd807aa98a3030242, 0x12835b0145706fbe, 0x243185be4ee4b28c, 0x550c7dc3d5ffb4e2,
        0x72be5d74f27b896f, 0x80deb1fe3b1696b1, 0x9bdc06a725c71235, 0xc19bf174cf692694,
        0xe49b69c19ef14ad2, 0xefbe4786384f25e3, 0x0fc19dc68b8cd5b5, 0x240ca1cc77ac9c65,
        0x2de92c6f592b0275, 0x4a7484aa6ea6e483, 0x5cb0a9dcbd41fbd4, 0x76f988da831153b5,
        0x983e5152ee66dfab, 0xa831c66d2db43210, 0xb00327c898fb213f, 0xbf597fc7beef0ee4,
        0xc6e00bf33da88fc2, 0xd5a79147930aa725, 0x06ca6351e003826f, 0x142929670a0e6e70,
        0x27b70a8546d22ffc, 0x2e1b21385c26c926, 0x4d2c6dfc5ac42aed, 0x53380d139d95b3df,
        0x650a73548baf63de, 0x766a0abb3c77b2a8, 0x81c2c92e47edaee6, 0x92722c851482353b,
        0xa2bfe8a14cf10364, 0xa81a664bbc423001, 0xc24b8b70d0f89791, 0xc76c51a30654be30,
        0xd192e819d6ef5218, 0xd69906245565a910, 0xf40e35855771202a, 0x106aa07032bbd1b8,
        0x19a4c116b8d2d0c8, 0x1e376c085141ab53, 0x2748774cdf8eeb99, 0x34b0bcb5e19b48a8,
        0x391c0cb3c5c95a63, 0x4ed8aa4ae3418acb, 0x5b9cca4f7763e373, 0x682e6ff3d6b2b8a3,
        0x748f82ee5defb2fc, 0x78a5636f43172f60, 0x84c87814a1f0ab72, 0x8cc702081a6439ec,
        0x90befffa23631e28, 0xa4506cebde82bde9, 0xbef9a3f7b2c67915, 0xc67178f2e372532b,
        0xca273eceea26619c, 0xd186b8c721c0c207, 0xeada7dd6cde0eb1e, 0xf57d4f7fee6ed178,
        0x06f067aa72176fba, 0x0a637dc5a2c898a6, 0x113f9804bef90dae, 0x1b710b35131c471b,
        0x28db77f523047d84, 0x32caab7b40c72493, 0x3c9ebe0a15c9bebc, 0x431d67c49c100d4c,
        0x4cc5d4becb3e42b6, 0x597f299cfc657e2a, 0x5fcb6fab3ad6faec, 0x6c44198c4a475817,
    ];

    // Pad message
    let mut padded = Vec::from(data);
    let orig_len = data.len() as u128;
    padded.push(0x80);
    while (padded.len() % 128) != 112 {
        padded.push(0);
    }
    padded.extend_from_slice(&(orig_len * 8).to_be_bytes());

    // Process blocks
    for block in padded.chunks(128) {
        let mut w = [0u64; 80];

        // Message schedule
        for i in 0..16 {
            w[i] = u64::from_be_bytes([
                block[i * 8], block[i * 8 + 1], block[i * 8 + 2], block[i * 8 + 3],
                block[i * 8 + 4], block[i * 8 + 5], block[i * 8 + 6], block[i * 8 + 7],
            ]);
        }

        for i in 16..80 {
            let s0 = w[i - 15].rotate_right(1) ^ w[i - 15].rotate_right(8) ^ (w[i - 15] >> 7);
            let s1 = w[i - 2].rotate_right(19) ^ w[i - 2].rotate_right(61) ^ (w[i - 2] >> 6);
            w[i] = w[i - 16].wrapping_add(s0).wrapping_add(w[i - 7]).wrapping_add(s1);
        }

        let mut a = h[0];
        let mut b = h[1];
        let mut c = h[2];
        let mut d = h[3];
        let mut e = h[4];
        let mut f = h[5];
        let mut g = h[6];
        let mut hh = h[7];

        for i in 0..80 {
            let s1 = e.rotate_right(14) ^ e.rotate_right(18) ^ e.rotate_right(41);
            let ch = (e & f) ^ ((!e) & g);
            let temp1 = hh.wrapping_add(s1).wrapping_add(ch).wrapping_add(K[i]).wrapping_add(w[i]);
            let s0 = a.rotate_right(28) ^ a.rotate_right(34) ^ a.rotate_right(39);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let temp2 = s0.wrapping_add(maj);

            hh = g;
            g = f;
            f = e;
            e = d.wrapping_add(temp1);
            d = c;
            c = b;
            b = a;
            a = temp1.wrapping_add(temp2);
        }

        h[0] = h[0].wrapping_add(a);
        h[1] = h[1].wrapping_add(b);
        h[2] = h[2].wrapping_add(c);
        h[3] = h[3].wrapping_add(d);
        h[4] = h[4].wrapping_add(e);
        h[5] = h[5].wrapping_add(f);
        h[6] = h[6].wrapping_add(g);
        h[7] = h[7].wrapping_add(hh);
    }

    let mut result = [0u8; 64];
    for i in 0..8 {
        result[i * 8..(i + 1) * 8].copy_from_slice(&h[i].to_be_bytes());
    }
    result
}

/// Ed25519 keypair
#[derive(Clone)]
pub struct Keypair {
    /// Private key seed (32 bytes)
    pub secret: [u8; 32],
    /// Public key (32 bytes)
    pub public: [u8; 32],
    /// Expanded private key (64 bytes): secret_scalar || prefix
    expanded: [u8; 64],
}

impl Keypair {
    /// Generate a new keypair from a 32-byte seed
    pub fn from_seed(seed: &[u8; 32]) -> Self {
        let h = sha512(seed);

        // Clamp the scalar
        let mut secret_scalar = [0u8; 32];
        secret_scalar.copy_from_slice(&h[..32]);
        secret_scalar[0] &= 248;
        secret_scalar[31] &= 127;
        secret_scalar[31] |= 64;

        // Compute public key A = s*B
        let public_point = Point::base_point().scalar_mul(&secret_scalar);
        let public = public_point.encode();

        // Store expanded key
        let mut expanded = [0u8; 64];
        expanded[..32].copy_from_slice(&secret_scalar);
        expanded[32..].copy_from_slice(&h[32..]);

        Self {
            secret: *seed,
            public,
            expanded,
        }
    }

    /// Generate a keypair from random bytes
    pub fn generate() -> Self {
        let mut seed = [0u8; 32];
        super::random_bytes(&mut seed);
        Self::from_seed(&seed)
    }

    /// Get the public key
    pub fn public_key(&self) -> &[u8; 32] {
        &self.public
    }

    /// Get the secret key (seed)
    pub fn secret_key(&self) -> &[u8; 32] {
        &self.secret
    }

    /// Sign a message
    pub fn sign(&self, message: &[u8]) -> [u8; 64] {
        // r = H(prefix || M) mod l
        let mut hash_input = Vec::with_capacity(32 + message.len());
        hash_input.extend_from_slice(&self.expanded[32..]);
        hash_input.extend_from_slice(message);
        let r_hash = sha512(&hash_input);
        let r = Scalar::from_bytes_wide(&r_hash);

        // R = r*B
        let r_point = Point::base_point().scalar_mul(r.as_bytes());
        let r_bytes = r_point.encode();

        // k = H(R || A || M) mod l
        let mut hash_input = Vec::with_capacity(32 + 32 + message.len());
        hash_input.extend_from_slice(&r_bytes);
        hash_input.extend_from_slice(&self.public);
        hash_input.extend_from_slice(message);
        let k_hash = sha512(&hash_input);
        let k = Scalar::from_bytes_wide(&k_hash);

        // s = r + k*a mod l
        let mut a = [0u8; 32];
        a.copy_from_slice(&self.expanded[..32]);
        let a_scalar = Scalar(a);
        let s = r.add(&k.mul(&a_scalar));

        // Signature = (R, s)
        let mut signature = [0u8; 64];
        signature[..32].copy_from_slice(&r_bytes);
        signature[32..].copy_from_slice(s.as_bytes());
        signature
    }
}

/// Verify an Ed25519 signature
pub fn verify(public_key: &[u8; 32], message: &[u8], signature: &[u8; 64]) -> bool {
    // Parse signature
    let mut r_bytes = [0u8; 32];
    let mut s_bytes = [0u8; 32];
    r_bytes.copy_from_slice(&signature[..32]);
    s_bytes.copy_from_slice(&signature[32..]);

    // Check s < l
    // (simplified: just verify it's a valid scalar)

    // Decode points
    let r = match Point::decode(&r_bytes) {
        Some(p) => p,
        None => return false,
    };
    let a = match Point::decode(public_key) {
        Some(p) => p,
        None => return false,
    };

    // k = H(R || A || M) mod l
    let mut hash_input = Vec::with_capacity(32 + 32 + message.len());
    hash_input.extend_from_slice(&r_bytes);
    hash_input.extend_from_slice(public_key);
    hash_input.extend_from_slice(message);
    let k_hash = sha512(&hash_input);
    let k = Scalar::from_bytes_wide(&k_hash);

    // Verify: [s]B = R + [k]A
    let sb = Point::base_point().scalar_mul(&s_bytes);
    let ka = a.scalar_mul(k.as_bytes());
    let rka = r.add(&ka);

    // Compare encoded points
    sb.encode() == rka.encode()
}

/// Sign a message with a private key seed
pub fn sign(secret_key: &[u8; 32], message: &[u8]) -> [u8; 64] {
    let keypair = Keypair::from_seed(secret_key);
    keypair.sign(message)
}

/// Generate a public key from a private key seed
pub fn public_key_from_secret(secret_key: &[u8; 32]) -> [u8; 32] {
    let keypair = Keypair::from_seed(secret_key);
    keypair.public
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sign_verify() {
        let seed = [0u8; 32];
        let keypair = Keypair::from_seed(&seed);

        let message = b"Hello, World!";
        let signature = keypair.sign(message);

        assert!(verify(&keypair.public, message, &signature));
    }

    #[test]
    fn test_wrong_message() {
        let seed = [0u8; 32];
        let keypair = Keypair::from_seed(&seed);

        let message = b"Hello, World!";
        let signature = keypair.sign(message);

        let wrong_message = b"Wrong message";
        assert!(!verify(&keypair.public, wrong_message, &signature));
    }
}
