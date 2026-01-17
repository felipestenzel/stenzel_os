//! Package Signing
//!
//! Ed25519 signature verification for packages.

use alloc::vec::Vec;
use crate::util::{KResult, KError};

/// Ed25519 public key size
pub const PUBLIC_KEY_SIZE: usize = 32;

/// Ed25519 signature size
pub const SIGNATURE_SIZE: usize = 64;

/// Ed25519 private key size (seed)
pub const PRIVATE_KEY_SIZE: usize = 32;

/// Repository public key for trusted packages
static mut TRUSTED_KEYS: [Option<[u8; PUBLIC_KEY_SIZE]>; 8] = [None; 8];

/// Number of trusted keys
static mut NUM_TRUSTED_KEYS: usize = 0;

/// Ed25519 field prime: 2^255 - 19
const FIELD_PRIME: [u64; 4] = [
    0xFFFFFFFFFFFFFFED,
    0xFFFFFFFFFFFFFFFF,
    0xFFFFFFFFFFFFFFFF,
    0x7FFFFFFFFFFFFFFF,
];

/// Ed25519 curve order
const CURVE_ORDER: [u64; 4] = [
    0x5812631A5CF5D3ED,
    0x14DEF9DEA2F79CD6,
    0x0000000000000000,
    0x1000000000000000,
];

/// Base point x-coordinate
const BASE_POINT_X: [u64; 4] = [
    0xC9562D608F25D51A,
    0x692CC7609525A7B2,
    0xC0A4E231FDD6DC5C,
    0x216936D3CD6E53FE,
];

/// Base point y-coordinate
const BASE_POINT_Y: [u64; 4] = [
    0x6666666666666658,
    0x6666666666666666,
    0x6666666666666666,
    0x6666666666666666,
];

/// Field element (256-bit)
#[derive(Clone, Copy)]
struct FieldElement([u64; 4]);

impl FieldElement {
    const ZERO: Self = Self([0, 0, 0, 0]);
    const ONE: Self = Self([1, 0, 0, 0]);

    /// Create from bytes (little-endian)
    fn from_bytes(bytes: &[u8]) -> Self {
        let mut result = [0u64; 4];
        for i in 0..4 {
            let start = i * 8;
            if start + 8 <= bytes.len() {
                result[i] = u64::from_le_bytes([
                    bytes[start], bytes[start + 1], bytes[start + 2], bytes[start + 3],
                    bytes[start + 4], bytes[start + 5], bytes[start + 6], bytes[start + 7],
                ]);
            } else if start < bytes.len() {
                let mut buf = [0u8; 8];
                let len = bytes.len() - start;
                buf[..len].copy_from_slice(&bytes[start..]);
                result[i] = u64::from_le_bytes(buf);
            }
        }
        Self(result)
    }

    /// Convert to bytes (little-endian)
    fn to_bytes(&self) -> [u8; 32] {
        let mut result = [0u8; 32];
        for i in 0..4 {
            let bytes = self.0[i].to_le_bytes();
            result[i * 8..(i + 1) * 8].copy_from_slice(&bytes);
        }
        result
    }

    /// Add two field elements
    fn add(&self, other: &Self) -> Self {
        let mut result = [0u64; 4];
        let mut carry = 0u64;

        for i in 0..4 {
            let (sum1, c1) = self.0[i].overflowing_add(other.0[i]);
            let (sum2, c2) = sum1.overflowing_add(carry);
            result[i] = sum2;
            carry = (c1 as u64) + (c2 as u64);
        }

        // Reduce modulo p if needed
        let mut reduced = Self(result);
        reduced.reduce();
        reduced
    }

    /// Subtract two field elements
    fn sub(&self, other: &Self) -> Self {
        let mut result = [0u64; 4];
        let mut borrow = 0u64;

        for i in 0..4 {
            let (diff1, b1) = self.0[i].overflowing_sub(other.0[i]);
            let (diff2, b2) = diff1.overflowing_sub(borrow);
            result[i] = diff2;
            borrow = (b1 as u64) + (b2 as u64);
        }

        // If we borrowed, add p back
        if borrow != 0 {
            let mut carry = 0u64;
            for i in 0..4 {
                let (sum1, c1) = result[i].overflowing_add(FIELD_PRIME[i]);
                let (sum2, c2) = sum1.overflowing_add(carry);
                result[i] = sum2;
                carry = (c1 as u64) + (c2 as u64);
            }
        }

        Self(result)
    }

    /// Multiply two field elements
    fn mul(&self, other: &Self) -> Self {
        // Full 512-bit multiplication result
        let mut product = [0u64; 8];

        for i in 0..4 {
            let mut carry = 0u128;
            for j in 0..4 {
                let p = (self.0[i] as u128) * (other.0[j] as u128) + (product[i + j] as u128) + carry;
                product[i + j] = p as u64;
                carry = p >> 64;
            }
            product[i + 4] = carry as u64;
        }

        // Reduce modulo p using Barrett reduction (simplified)
        Self::reduce_wide(&product)
    }

    /// Reduce a 512-bit number modulo p
    fn reduce_wide(product: &[u64; 8]) -> Self {
        // Simplified reduction for Ed25519 field
        // p = 2^255 - 19, so 2^255 ≡ 19 (mod p)

        let mut result = [0u64; 4];

        // Lower 255 bits
        for i in 0..4 {
            result[i] = product[i];
        }
        result[3] &= 0x7FFFFFFFFFFFFFFF;

        // Upper bits * 19
        let high_bit = (product[3] >> 63) & 1;
        let mut high = [0u64; 4];
        for i in 0..4 {
            high[i] = product[i + 4];
        }
        high[0] |= high_bit << 63;

        // Multiply by 38 (2 * 19) for bits 256-511
        let mut carry = 0u128;
        for i in 0..4 {
            let p = (high[i] as u128) * 38 + (result[i] as u128) + carry;
            result[i] = p as u64;
            carry = p >> 64;
        }

        // Handle any remaining carry
        while carry > 0 {
            let p = (carry as u64).wrapping_mul(38) as u128 + result[0] as u128;
            result[0] = p as u64;
            carry = p >> 64;
        }

        let mut fe = Self(result);
        fe.reduce();
        fe
    }

    /// Reduce to canonical form
    fn reduce(&mut self) {
        // Check if >= p
        let mut ge_p = true;
        for i in (0..4).rev() {
            if self.0[i] < FIELD_PRIME[i] {
                ge_p = false;
                break;
            } else if self.0[i] > FIELD_PRIME[i] {
                break;
            }
        }

        if ge_p {
            let mut borrow = 0u64;
            for i in 0..4 {
                let (diff1, b1) = self.0[i].overflowing_sub(FIELD_PRIME[i]);
                let (diff2, b2) = diff1.overflowing_sub(borrow);
                self.0[i] = diff2;
                borrow = (b1 as u64) + (b2 as u64);
            }
        }
    }

    /// Square a field element
    fn square(&self) -> Self {
        self.mul(self)
    }

    /// Compute modular inverse using Fermat's little theorem
    /// a^(-1) = a^(p-2) mod p
    fn invert(&self) -> Self {
        // p - 2 = 2^255 - 21
        let mut result = Self::ONE;
        let mut base = *self;

        // Binary exponentiation
        let exp: [u64; 4] = [
            0xFFFFFFFFFFFFFFEB,
            0xFFFFFFFFFFFFFFFF,
            0xFFFFFFFFFFFFFFFF,
            0x7FFFFFFFFFFFFFFF,
        ];

        for i in 0..4 {
            for j in 0..64 {
                if (exp[i] >> j) & 1 == 1 {
                    result = result.mul(&base);
                }
                base = base.square();
            }
        }

        result
    }
}

/// Edwards curve point
#[derive(Clone, Copy)]
struct Point {
    x: FieldElement,
    y: FieldElement,
    z: FieldElement,
    t: FieldElement,
}

impl Point {
    /// Identity point (neutral element)
    fn identity() -> Self {
        Self {
            x: FieldElement::ZERO,
            y: FieldElement::ONE,
            z: FieldElement::ONE,
            t: FieldElement::ZERO,
        }
    }

    /// Base point
    fn base_point() -> Self {
        Self {
            x: FieldElement(BASE_POINT_X),
            y: FieldElement(BASE_POINT_Y),
            z: FieldElement::ONE,
            t: FieldElement(BASE_POINT_X).mul(&FieldElement(BASE_POINT_Y)),
        }
    }

    /// Point addition (extended coordinates)
    fn add(&self, other: &Self) -> Self {
        // Using extended coordinates formulas
        let a = self.x.mul(&other.x);
        let b = self.y.mul(&other.y);
        let c = self.t.mul(&other.t).mul(&FieldElement([
            0x2406D9DC56DFFCE7,
            0x029BDEF93C1E8FE8,
            0x0000000000000000,
            0x0000000000000000,
        ])); // d * 2
        let d = self.z.mul(&other.z);
        let e = self.x.add(&self.y).mul(&other.x.add(&other.y)).sub(&a).sub(&b);
        let f = d.sub(&c);
        let g = d.add(&c);
        let h = b.sub(&a.mul(&FieldElement([
            0xFFFFFFFFFFFFFFFF,
            0xFFFFFFFFFFFFFFFF,
            0xFFFFFFFFFFFFFFFF,
            0x7FFFFFFFFFFFFFFF,
        ]))); // b - a*(-1) = b + a

        Self {
            x: e.mul(&f),
            y: g.mul(&h),
            z: f.mul(&g),
            t: e.mul(&h),
        }
    }

    /// Point doubling
    fn double(&self) -> Self {
        let a = self.x.square();
        let b = self.y.square();
        let c = self.z.square();
        let c2 = c.add(&c);
        let d = FieldElement([
            0xFFFFFFFFFFFFFFFF,
            0xFFFFFFFFFFFFFFFF,
            0xFFFFFFFFFFFFFFFF,
            0x7FFFFFFFFFFFFFFF,
        ]).mul(&a); // -a
        let e = self.x.add(&self.y).square().sub(&a).sub(&b);
        let g = d.add(&b);
        let f = g.sub(&c2);
        let h = d.sub(&b);

        Self {
            x: e.mul(&f),
            y: g.mul(&h),
            z: f.mul(&g),
            t: e.mul(&h),
        }
    }

    /// Scalar multiplication
    fn scalar_mul(&self, scalar: &[u8; 32]) -> Self {
        let mut result = Point::identity();
        let mut temp = *self;

        for i in 0..256 {
            let byte_idx = i / 8;
            let bit_idx = i % 8;
            if (scalar[byte_idx] >> bit_idx) & 1 == 1 {
                result = result.add(&temp);
            }
            temp = temp.double();
        }

        result
    }

    /// Convert to affine coordinates and encode
    fn encode(&self) -> [u8; 32] {
        let z_inv = self.z.invert();
        let x = self.x.mul(&z_inv);
        let y = self.y.mul(&z_inv);

        let mut result = y.to_bytes();
        // Set high bit to sign of x
        result[31] |= ((x.0[0] & 1) << 7) as u8;
        result
    }

    /// Decode point from bytes
    fn decode(bytes: &[u8; 32]) -> Option<Self> {
        let mut y_bytes = *bytes;
        let x_sign = (y_bytes[31] >> 7) & 1;
        y_bytes[31] &= 0x7F;

        let y = FieldElement::from_bytes(&y_bytes);

        // Compute x^2 = (y^2 - 1) / (d*y^2 + 1)
        let y2 = y.square();
        let d = FieldElement([
            0x135978A3785913A3,
            0x75EB4DCA135978A3,
            0x00000000A0B5BFF,
            0x6A6A3D29A3C9F6A,
        ]);

        let num = y2.sub(&FieldElement::ONE);
        let den = d.mul(&y2).add(&FieldElement::ONE);
        let den_inv = den.invert();
        let x2 = num.mul(&den_inv);

        // Square root (simplified)
        // x = x2^((p+3)/8) with correction if needed
        let x = sqrt_ratio(&x2);

        // Verify sign
        let computed_sign = (x.0[0] & 1) as u8;
        let x = if computed_sign != x_sign {
            FieldElement::ZERO.sub(&x)
        } else {
            x
        };

        Some(Self {
            x,
            y,
            z: FieldElement::ONE,
            t: x.mul(&y),
        })
    }
}

/// Square root for Ed25519 field
fn sqrt_ratio(u: &FieldElement) -> FieldElement {
    // Simplified: u^((p+3)/8)
    let exp: [u64; 4] = [
        0xFFFFFFFFFFFFFFFE,
        0xFFFFFFFFFFFFFFFF,
        0xFFFFFFFFFFFFFFFF,
        0x0FFFFFFFFFFFFFFF,
    ];

    let mut result = FieldElement::ONE;
    let mut base = *u;

    for i in 0..4 {
        for j in 0..64 {
            if (exp[i] >> j) & 1 == 1 {
                result = result.mul(&base);
            }
            base = base.square();
        }
    }

    result
}

/// SHA-512 hash (simplified implementation)
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

/// Verify an Ed25519 signature
pub fn verify_signature(message: &[u8], signature: &[u8], public_key: &[u8]) -> KResult<bool> {
    if signature.len() != SIGNATURE_SIZE {
        return Err(KError::Invalid);
    }
    if public_key.len() != PUBLIC_KEY_SIZE {
        return Err(KError::Invalid);
    }

    // Parse signature
    let mut r_bytes = [0u8; 32];
    let mut s_bytes = [0u8; 32];
    r_bytes.copy_from_slice(&signature[..32]);
    s_bytes.copy_from_slice(&signature[32..]);

    // Decode points
    let r = Point::decode(&r_bytes).ok_or(KError::Invalid)?;
    let mut pk_bytes = [0u8; 32];
    pk_bytes.copy_from_slice(public_key);
    let a = Point::decode(&pk_bytes).ok_or(KError::Invalid)?;

    // Compute h = SHA512(R || A || M) mod l
    let mut hash_input = Vec::new();
    hash_input.extend_from_slice(&r_bytes);
    hash_input.extend_from_slice(public_key);
    hash_input.extend_from_slice(message);
    let hash = sha512(&hash_input);

    // Reduce hash mod curve order (simplified)
    let mut h_scalar = [0u8; 32];
    h_scalar.copy_from_slice(&hash[..32]);

    // Verify: [s]B = R + [h]A
    let sb = Point::base_point().scalar_mul(&s_bytes);
    let ha = a.scalar_mul(&h_scalar);
    let rha = r.add(&ha);

    // Compare encoded points
    let sb_encoded = sb.encode();
    let rha_encoded = rha.encode();

    Ok(sb_encoded == rha_encoded)
}

/// Add a trusted public key
pub fn add_trusted_key(key: &[u8; PUBLIC_KEY_SIZE]) -> KResult<()> {
    unsafe {
        if NUM_TRUSTED_KEYS >= 8 {
            return Err(KError::NoMemory);
        }
        TRUSTED_KEYS[NUM_TRUSTED_KEYS] = Some(*key);
        NUM_TRUSTED_KEYS += 1;
    }
    Ok(())
}

/// Check if signature is from a trusted key
pub fn verify_trusted_signature(message: &[u8], signature: &[u8]) -> KResult<bool> {
    unsafe {
        for i in 0..NUM_TRUSTED_KEYS {
            if let Some(key) = &TRUSTED_KEYS[i] {
                if verify_signature(message, signature, key)? {
                    return Ok(true);
                }
            }
        }
    }
    Ok(false)
}

/// Initialize signing subsystem
pub fn init() -> KResult<()> {
    crate::kprintln!("spkg: Ed25519 signature verification initialized");
    Ok(())
}

/// Load trusted keys from file
pub fn load_trusted_keys() -> KResult<()> {
    // In a full implementation, this would:
    // 1. Read /etc/spkg/keys/ directory
    // 2. Load each .pub file
    // 3. Parse and add to trusted keys
    Ok(())
}
