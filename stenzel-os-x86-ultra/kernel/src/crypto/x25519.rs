//! X25519 Key Exchange
//!
//! Implementation of Curve25519 Diffie-Hellman key exchange (RFC 7748).

#![allow(dead_code)]

/// Field element in GF(2^255-19) represented as 10 limbs of 25.5 bits
#[derive(Clone, Copy)]
struct Fe([i64; 10]);

impl Fe {
    const ZERO: Self = Self([0; 10]);
    const ONE: Self = Self([1, 0, 0, 0, 0, 0, 0, 0, 0, 0]);

    /// Load from 32 bytes (little-endian)
    fn from_bytes(s: &[u8; 32]) -> Self {
        let mut h = [0i64; 10];

        h[0] = load4(&s[0..]) as i64;
        h[1] = (load3(&s[4..]) << 6) as i64;
        h[2] = (load3(&s[7..]) << 5) as i64;
        h[3] = (load3(&s[10..]) << 3) as i64;
        h[4] = (load3(&s[13..]) << 2) as i64;
        h[5] = load4(&s[16..]) as i64;
        h[6] = (load3(&s[20..]) << 7) as i64;
        h[7] = (load3(&s[23..]) << 5) as i64;
        h[8] = (load3(&s[26..]) << 4) as i64;
        h[9] = ((load3(&s[29..]) & 0x7fffff) << 2) as i64;

        Self(h)
    }

    /// Store to 32 bytes (little-endian)
    fn to_bytes(&self) -> [u8; 32] {
        let mut h = self.0;

        let mut q = (19 * h[9] + (1 << 24)) >> 25;
        q = (h[0] + q) >> 26;
        q = (h[1] + q) >> 25;
        q = (h[2] + q) >> 26;
        q = (h[3] + q) >> 25;
        q = (h[4] + q) >> 26;
        q = (h[5] + q) >> 25;
        q = (h[6] + q) >> 26;
        q = (h[7] + q) >> 25;
        q = (h[8] + q) >> 26;
        q = (h[9] + q) >> 25;

        h[0] += 19 * q;

        let carry0 = h[0] >> 26;
        h[1] += carry0;
        h[0] -= carry0 << 26;

        let carry1 = h[1] >> 25;
        h[2] += carry1;
        h[1] -= carry1 << 25;

        let carry2 = h[2] >> 26;
        h[3] += carry2;
        h[2] -= carry2 << 26;

        let carry3 = h[3] >> 25;
        h[4] += carry3;
        h[3] -= carry3 << 25;

        let carry4 = h[4] >> 26;
        h[5] += carry4;
        h[4] -= carry4 << 26;

        let carry5 = h[5] >> 25;
        h[6] += carry5;
        h[5] -= carry5 << 25;

        let carry6 = h[6] >> 26;
        h[7] += carry6;
        h[6] -= carry6 << 26;

        let carry7 = h[7] >> 25;
        h[8] += carry7;
        h[7] -= carry7 << 25;

        let carry8 = h[8] >> 26;
        h[9] += carry8;
        h[8] -= carry8 << 26;

        let carry9 = h[9] >> 25;
        h[9] -= carry9 << 25;

        let mut s = [0u8; 32];
        s[0] = h[0] as u8;
        s[1] = (h[0] >> 8) as u8;
        s[2] = (h[0] >> 16) as u8;
        s[3] = ((h[0] >> 24) | (h[1] << 2)) as u8;
        s[4] = (h[1] >> 6) as u8;
        s[5] = (h[1] >> 14) as u8;
        s[6] = ((h[1] >> 22) | (h[2] << 3)) as u8;
        s[7] = (h[2] >> 5) as u8;
        s[8] = (h[2] >> 13) as u8;
        s[9] = ((h[2] >> 21) | (h[3] << 5)) as u8;
        s[10] = (h[3] >> 3) as u8;
        s[11] = (h[3] >> 11) as u8;
        s[12] = ((h[3] >> 19) | (h[4] << 6)) as u8;
        s[13] = (h[4] >> 2) as u8;
        s[14] = (h[4] >> 10) as u8;
        s[15] = (h[4] >> 18) as u8;
        s[16] = h[5] as u8;
        s[17] = (h[5] >> 8) as u8;
        s[18] = (h[5] >> 16) as u8;
        s[19] = ((h[5] >> 24) | (h[6] << 1)) as u8;
        s[20] = (h[6] >> 7) as u8;
        s[21] = (h[6] >> 15) as u8;
        s[22] = ((h[6] >> 23) | (h[7] << 3)) as u8;
        s[23] = (h[7] >> 5) as u8;
        s[24] = (h[7] >> 13) as u8;
        s[25] = ((h[7] >> 21) | (h[8] << 4)) as u8;
        s[26] = (h[8] >> 4) as u8;
        s[27] = (h[8] >> 12) as u8;
        s[28] = ((h[8] >> 20) | (h[9] << 6)) as u8;
        s[29] = (h[9] >> 2) as u8;
        s[30] = (h[9] >> 10) as u8;
        s[31] = (h[9] >> 18) as u8;

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
            self.0[5] + b.0[5],
            self.0[6] + b.0[6],
            self.0[7] + b.0[7],
            self.0[8] + b.0[8],
            self.0[9] + b.0[9],
        ])
    }

    /// Subtract two field elements
    fn sub(&self, b: &Self) -> Self {
        Self([
            self.0[0] - b.0[0],
            self.0[1] - b.0[1],
            self.0[2] - b.0[2],
            self.0[3] - b.0[3],
            self.0[4] - b.0[4],
            self.0[5] - b.0[5],
            self.0[6] - b.0[6],
            self.0[7] - b.0[7],
            self.0[8] - b.0[8],
            self.0[9] - b.0[9],
        ])
    }

    /// Multiply two field elements
    fn mul(&self, b: &Self) -> Self {
        let f0 = self.0[0];
        let f1 = self.0[1];
        let f2 = self.0[2];
        let f3 = self.0[3];
        let f4 = self.0[4];
        let f5 = self.0[5];
        let f6 = self.0[6];
        let f7 = self.0[7];
        let f8 = self.0[8];
        let f9 = self.0[9];

        let g0 = b.0[0];
        let g1 = b.0[1];
        let g2 = b.0[2];
        let g3 = b.0[3];
        let g4 = b.0[4];
        let g5 = b.0[5];
        let g6 = b.0[6];
        let g7 = b.0[7];
        let g8 = b.0[8];
        let g9 = b.0[9];

        let g1_19 = 19 * g1;
        let g2_19 = 19 * g2;
        let g3_19 = 19 * g3;
        let g4_19 = 19 * g4;
        let g5_19 = 19 * g5;
        let g6_19 = 19 * g6;
        let g7_19 = 19 * g7;
        let g8_19 = 19 * g8;
        let g9_19 = 19 * g9;

        let f1_2 = 2 * f1;
        let f3_2 = 2 * f3;
        let f5_2 = 2 * f5;
        let f7_2 = 2 * f7;
        let f9_2 = 2 * f9;

        let h0 = f0 * g0 + f1_2 * g9_19 + f2 * g8_19 + f3_2 * g7_19 + f4 * g6_19 + f5_2 * g5_19 + f6 * g4_19 + f7_2 * g3_19 + f8 * g2_19 + f9_2 * g1_19;
        let h1 = f0 * g1 + f1 * g0 + f2 * g9_19 + f3 * g8_19 + f4 * g7_19 + f5 * g6_19 + f6 * g5_19 + f7 * g4_19 + f8 * g3_19 + f9 * g2_19;
        let h2 = f0 * g2 + f1_2 * g1 + f2 * g0 + f3_2 * g9_19 + f4 * g8_19 + f5_2 * g7_19 + f6 * g6_19 + f7_2 * g5_19 + f8 * g4_19 + f9_2 * g3_19;
        let h3 = f0 * g3 + f1 * g2 + f2 * g1 + f3 * g0 + f4 * g9_19 + f5 * g8_19 + f6 * g7_19 + f7 * g6_19 + f8 * g5_19 + f9 * g4_19;
        let h4 = f0 * g4 + f1_2 * g3 + f2 * g2 + f3_2 * g1 + f4 * g0 + f5_2 * g9_19 + f6 * g8_19 + f7_2 * g7_19 + f8 * g6_19 + f9_2 * g5_19;
        let h5 = f0 * g5 + f1 * g4 + f2 * g3 + f3 * g2 + f4 * g1 + f5 * g0 + f6 * g9_19 + f7 * g8_19 + f8 * g7_19 + f9 * g6_19;
        let h6 = f0 * g6 + f1_2 * g5 + f2 * g4 + f3_2 * g3 + f4 * g2 + f5_2 * g1 + f6 * g0 + f7_2 * g9_19 + f8 * g8_19 + f9_2 * g7_19;
        let h7 = f0 * g7 + f1 * g6 + f2 * g5 + f3 * g4 + f4 * g3 + f5 * g2 + f6 * g1 + f7 * g0 + f8 * g9_19 + f9 * g8_19;
        let h8 = f0 * g8 + f1_2 * g7 + f2 * g6 + f3_2 * g5 + f4 * g4 + f5_2 * g3 + f6 * g2 + f7_2 * g1 + f8 * g0 + f9_2 * g9_19;
        let h9 = f0 * g9 + f1 * g8 + f2 * g7 + f3 * g6 + f4 * g5 + f5 * g4 + f6 * g3 + f7 * g2 + f8 * g1 + f9 * g0;

        Self::carry([h0, h1, h2, h3, h4, h5, h6, h7, h8, h9])
    }

    /// Square a field element
    fn square(&self) -> Self {
        let f0 = self.0[0];
        let f1 = self.0[1];
        let f2 = self.0[2];
        let f3 = self.0[3];
        let f4 = self.0[4];
        let f5 = self.0[5];
        let f6 = self.0[6];
        let f7 = self.0[7];
        let f8 = self.0[8];
        let f9 = self.0[9];

        let f0_2 = 2 * f0;
        let f1_2 = 2 * f1;
        let f2_2 = 2 * f2;
        let f3_2 = 2 * f3;
        let f4_2 = 2 * f4;
        let f5_2 = 2 * f5;
        let f6_2 = 2 * f6;
        let f7_2 = 2 * f7;

        let f5_38 = 38 * f5;
        let f6_19 = 19 * f6;
        let f7_38 = 38 * f7;
        let f8_19 = 19 * f8;
        let f9_38 = 38 * f9;

        let h0 = f0 * f0 + f1_2 * f9_38 + f2_2 * f8_19 + f3_2 * f7_38 + f4_2 * f6_19 + f5 * f5_38;
        let h1 = f0_2 * f1 + f2 * f9_38 + f3_2 * f8_19 + f4 * f7_38 + f5_2 * f6_19;
        let h2 = f0_2 * f2 + f1_2 * f1 + f3_2 * f9_38 + f4_2 * f8_19 + f5_2 * f7_38 + f6 * f6_19;
        let h3 = f0_2 * f3 + f1_2 * f2 + f4 * f9_38 + f5_2 * f8_19 + f6_2 * f7_38;
        let h4 = f0_2 * f4 + f1_2 * f3_2 + f2 * f2 + f5_2 * f9_38 + f6_2 * f8_19 + f7 * f7_38;
        let h5 = f0_2 * f5 + f1_2 * f4 + f2_2 * f3 + f6 * f9_38 + f7_2 * f8_19;
        let h6 = f0_2 * f6 + f1_2 * f5_2 + f2_2 * f4 + f3_2 * f3 + f7_2 * f9_38 + f8 * f8_19;
        let h7 = f0_2 * f7 + f1_2 * f6 + f2_2 * f5 + f3_2 * f4 + f8 * f9_38;
        let h8 = f0_2 * f8 + f1_2 * f7_2 + f2_2 * f6 + f3_2 * f5_2 + f4 * f4 + f9 * f9_38;
        let h9 = f0_2 * f9 + f1_2 * f8 + f2_2 * f7 + f3_2 * f6 + f4_2 * f5;

        Self::carry([h0, h1, h2, h3, h4, h5, h6, h7, h8, h9])
    }

    /// Carry and reduce
    fn carry(h: [i64; 10]) -> Self {
        let mut out = [0i64; 10];

        let mut carry = (h[0] + (1 << 25)) >> 26;
        out[0] = h[0] - (carry << 26);
        let h1 = h[1] + carry;

        carry = (h1 + (1 << 24)) >> 25;
        out[1] = h1 - (carry << 25);
        let h2 = h[2] + carry;

        carry = (h2 + (1 << 25)) >> 26;
        out[2] = h2 - (carry << 26);
        let h3 = h[3] + carry;

        carry = (h3 + (1 << 24)) >> 25;
        out[3] = h3 - (carry << 25);
        let h4 = h[4] + carry;

        carry = (h4 + (1 << 25)) >> 26;
        out[4] = h4 - (carry << 26);
        let h5 = h[5] + carry;

        carry = (h5 + (1 << 24)) >> 25;
        out[5] = h5 - (carry << 25);
        let h6 = h[6] + carry;

        carry = (h6 + (1 << 25)) >> 26;
        out[6] = h6 - (carry << 26);
        let h7 = h[7] + carry;

        carry = (h7 + (1 << 24)) >> 25;
        out[7] = h7 - (carry << 25);
        let h8 = h[8] + carry;

        carry = (h8 + (1 << 25)) >> 26;
        out[8] = h8 - (carry << 26);
        let h9 = h[9] + carry;

        carry = (h9 + (1 << 24)) >> 25;
        out[9] = h9 - (carry << 25);
        out[0] += carry * 19;

        carry = out[0] >> 26;
        out[0] &= 0x3ffffff;
        out[1] += carry;

        Self(out)
    }

    /// Invert a field element (using Fermat's little theorem)
    fn invert(&self) -> Self {
        let z2 = self.square();
        let z4 = z2.square();
        let z8 = z4.square();
        let z9 = self.mul(&z8);
        let z11 = z2.mul(&z9);
        let z22 = z11.square();
        let z_5_0 = z9.mul(&z22);

        let mut t0 = z_5_0.square();
        let mut t1 = t0.square();
        t1 = t1.square();
        t1 = t1.square();
        t1 = t1.square();
        t0 = t1.mul(&z_5_0);

        t1 = t0.square();
        for _ in 1..10 {
            t1 = t1.square();
        }
        t0 = t1.mul(&t0);

        t1 = t0.square();
        for _ in 1..20 {
            t1 = t1.square();
        }
        t1 = t1.mul(&t0);

        t1 = t1.square();
        for _ in 1..10 {
            t1 = t1.square();
        }
        t0 = t1.mul(&t0);

        t1 = t0.square();
        for _ in 1..50 {
            t1 = t1.square();
        }
        t1 = t1.mul(&t0);

        let mut t2 = t1.square();
        for _ in 1..100 {
            t2 = t2.square();
        }
        t1 = t2.mul(&t1);

        t1 = t1.square();
        for _ in 1..50 {
            t1 = t1.square();
        }
        t0 = t1.mul(&t0);

        t0 = t0.square();
        t0 = t0.square();
        t0 = t0.square();
        t0 = t0.square();
        t0 = t0.square();

        t0.mul(&z11)
    }

    /// Conditional swap
    fn cswap(&mut self, other: &mut Self, swap: i64) {
        let swap = -swap;
        for i in 0..10 {
            let t = swap & (self.0[i] ^ other.0[i]);
            self.0[i] ^= t;
            other.0[i] ^= t;
        }
    }
}

/// Load 4 bytes little-endian
fn load4(s: &[u8]) -> u64 {
    (s[0] as u64) | ((s[1] as u64) << 8) | ((s[2] as u64) << 16) | ((s[3] as u64) << 24)
}

/// Load 3 bytes little-endian
fn load3(s: &[u8]) -> u64 {
    (s[0] as u64) | ((s[1] as u64) << 8) | ((s[2] as u64) << 16)
}

/// X25519 base point (generator)
const BASEPOINT: [u8; 32] = [9, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];

/// X25519 scalar multiplication
pub fn x25519(k: &[u8; 32], u: &[u8; 32]) -> [u8; 32] {
    // Clamp the scalar
    let mut k = *k;
    k[0] &= 248;
    k[31] &= 127;
    k[31] |= 64;

    // Montgomery ladder
    let x1 = Fe::from_bytes(u);
    let mut x2 = Fe::ONE;
    let mut z2 = Fe::ZERO;
    let mut x3 = x1;
    let mut z3 = Fe::ONE;

    let mut swap: i64 = 0;

    for i in (0..255).rev() {
        let ki = ((k[i / 8] >> (i & 7)) & 1) as i64;
        swap ^= ki;
        x2.cswap(&mut x3, swap);
        z2.cswap(&mut z3, swap);
        swap = ki;

        let a = x2.add(&z2);
        let aa = a.square();
        let b = x2.sub(&z2);
        let bb = b.square();
        let e = aa.sub(&bb);
        let c = x3.add(&z3);
        let d = x3.sub(&z3);
        let da = d.mul(&a);
        let cb = c.mul(&b);
        let x3_new = da.add(&cb).square();
        let z3_new = x1.mul(&da.sub(&cb).square());

        x2 = aa.mul(&bb);
        z2 = e.mul(&aa.add(&e.mul(&Fe([121666, 0, 0, 0, 0, 0, 0, 0, 0, 0]))));
        x3 = x3_new;
        z3 = z3_new;
    }

    x2.cswap(&mut x3, swap);
    z2.cswap(&mut z3, swap);

    x2.mul(&z2.invert()).to_bytes()
}

/// Generate X25519 public key from private key
pub fn x25519_public_key(private_key: &[u8; 32]) -> [u8; 32] {
    x25519(private_key, &BASEPOINT)
}

/// Perform X25519 Diffie-Hellman
pub fn x25519_diffie_hellman(private_key: &[u8; 32], public_key: &[u8; 32]) -> [u8; 32] {
    x25519(private_key, public_key)
}
