//! FarmHash-compatible string hashing.
//!
//! This module provides a hash function compatible with OIIO's
//! `Strutil::strhash`, which uses Google FarmHash (specifically
//! `farmhash::Fingerprint64`). Fingerprint functions are stable across
//! platforms and library versions, producing identical results everywhere.
//!
//! The implementation is a direct port of the relevant parts of
//! `farmhash.cc` (Fingerprint64 path). All functions are `const fn`.

/// Compute a 64-bit FarmHash fingerprint of a byte slice.
/// This is deterministic and stable across platforms, matching
/// `farmhash::Fingerprint64`.
pub const fn fingerprint64(s: &[u8]) -> u64 {
    let len = s.len();
    if len <= 16 {
        hash_len_0_to_16(s)
    } else if len <= 32 {
        hash_len_17_to_32(s)
    } else if len <= 64 {
        hash_len_33_to_64(s)
    } else {
        hash_len_65_plus(s)
    }
}

/// Convenience: hash a string at compile time or runtime.
#[inline]
pub const fn strhash(s: &str) -> u64 {
    fingerprint64(s.as_bytes())
}

/// Convenience: hash a byte slice.
#[inline]
pub const fn strhash_bytes(s: &[u8]) -> u64 {
    fingerprint64(s)
}

/// OSL builtin `hash(string)` -- truncates FarmHash to i32.
#[inline]
pub const fn oslhash(s: &str) -> i32 {
    strhash(s) as i32
}

// ---------------------------------------------------------------------------
// Bob Jenkins lookup3 hash (for noise coordinate hashing)
// ---------------------------------------------------------------------------

/// Jenkins lookup3 bjmix: mix three u32 values in-place.
#[inline]
pub fn bjmix(a: &mut u32, b: &mut u32, c: &mut u32) {
    *a = a.wrapping_sub(*c);
    *a ^= c.rotate_left(4);
    *c = c.wrapping_add(*b);
    *b = b.wrapping_sub(*a);
    *b ^= a.rotate_left(6);
    *a = a.wrapping_add(*c);
    *c = c.wrapping_sub(*b);
    *c ^= b.rotate_left(8);
    *b = b.wrapping_add(*a);
    *a = a.wrapping_sub(*c);
    *a ^= c.rotate_left(16);
    *c = c.wrapping_add(*b);
    *b = b.wrapping_sub(*a);
    *b ^= a.rotate_left(19);
    *a = a.wrapping_add(*c);
    *c = c.wrapping_sub(*b);
    *c ^= b.rotate_left(4);
    *b = b.wrapping_add(*a);
}

/// Jenkins lookup3 bjfinal: final avalanche mix.
#[inline]
pub fn bjfinal(a_in: u32, b_in: u32, c_in: u32) -> u32 {
    let (mut a, mut b, mut c) = (a_in, b_in, c_in);
    c ^= b;
    c = c.wrapping_sub(b.rotate_left(14));
    a ^= c;
    a = a.wrapping_sub(c.rotate_left(11));
    b ^= a;
    b = b.wrapping_sub(a.rotate_left(25));
    c ^= b;
    c = c.wrapping_sub(b.rotate_left(16));
    a ^= c;
    a = a.wrapping_sub(c.rotate_left(4));
    b ^= a;
    b = b.wrapping_sub(a.rotate_left(14));
    c ^= b;
    c = c.wrapping_sub(b.rotate_left(24));
    c
}

/// Hash 1 integer via Bob Jenkins lookup3 (matches C++ OSL `inthash(k0)`).
#[inline]
pub fn inthash1(k0: u32) -> u32 {
    let start = 0xdeadbeefu32.wrapping_add(1 << 2).wrapping_add(13);
    let a = start.wrapping_add(k0);
    bjfinal(a, start, start)
}

/// Hash 2 integers via Bob Jenkins lookup3 (matches C++ OSL `inthash(k0,k1)`).
#[inline]
pub fn inthash2(k0: u32, k1: u32) -> u32 {
    let start = 0xdeadbeefu32.wrapping_add(2 << 2).wrapping_add(13);
    let a = start.wrapping_add(k0);
    let b = start.wrapping_add(k1);
    bjfinal(a, b, start)
}

/// Hash 3 integers (matches C++ OSL `inthash(k0,k1,k2)`).
#[inline]
pub fn inthash3(k0: u32, k1: u32, k2: u32) -> u32 {
    let start = 0xdeadbeefu32.wrapping_add(3 << 2).wrapping_add(13);
    let a = start.wrapping_add(k0);
    let b = start.wrapping_add(k1);
    let c = start.wrapping_add(k2);
    bjfinal(a, b, c)
}

/// Hash 4 integers (matches C++ OSL `inthash(k0,k1,k2,k3)`).
#[inline]
pub fn inthash4(k0: u32, k1: u32, k2: u32, k3: u32) -> u32 {
    let start = 0xdeadbeefu32.wrapping_add(4 << 2).wrapping_add(13);
    let mut a = start.wrapping_add(k0);
    let mut b = start.wrapping_add(k1);
    let mut c = start.wrapping_add(k2);
    bjmix(&mut a, &mut b, &mut c);
    a = a.wrapping_add(k3);
    bjfinal(a, b, c)
}

/// Hash 5 integers (matches C++ OSL `inthash(k0..k4)`).
#[inline]
pub fn inthash5(k0: u32, k1: u32, k2: u32, k3: u32, k4: u32) -> u32 {
    let start = 0xdeadbeefu32.wrapping_add(5 << 2).wrapping_add(13);
    let mut a = start.wrapping_add(k0);
    let mut b = start.wrapping_add(k1);
    let mut c = start.wrapping_add(k2);
    bjmix(&mut a, &mut b, &mut c);
    b = b.wrapping_add(k4);
    a = a.wrapping_add(k3);
    bjfinal(a, b, c)
}

/// Convert a u32 hash to a float in [0, 1].
/// Matches C++ OSL `bits_to_01`.
#[inline]
pub fn bits_to_01(bits: u32) -> f32 {
    // 1.0 / u32::MAX, computed in f64 for precision
    const FACTOR: f32 = (1.0f64 / u32::MAX as f64) as f32;
    bits as f32 * FACTOR
}

/// Hash 3 integers into a Vec3 via Bob Jenkins lookup3.
/// Optimized: performs `bjmix` once, then 3 `bjfinal` calls with seed 0/1/2.
/// Matches C++ OSL `inthashVec(k0, k1, k2)`.
#[inline]
pub fn inthash_vec3(k0: u32, k1: u32, k2: u32) -> [f32; 3] {
    let start = 0xdeadbeefu32.wrapping_add(4 << 2).wrapping_add(13);
    let mut a = start.wrapping_add(k0);
    let mut b = start.wrapping_add(k1);
    let mut c = start.wrapping_add(k2);
    bjmix(&mut a, &mut b, &mut c);
    [
        bits_to_01(bjfinal(a, b, c)),
        bits_to_01(bjfinal(a.wrapping_add(1), b, c)),
        bits_to_01(bjfinal(a.wrapping_add(2), b, c)),
    ]
}

/// Hash 4 integers into a Vec3 via Bob Jenkins lookup3.
/// Matches C++ OSL `inthashVec(k0, k1, k2, k3)`.
#[inline]
pub fn inthash_vec4(k0: u32, k1: u32, k2: u32, k3: u32) -> [f32; 3] {
    let start = 0xdeadbeefu32.wrapping_add(5 << 2).wrapping_add(13);
    let mut a = start.wrapping_add(k0);
    let mut b = start.wrapping_add(k1);
    let mut c = start.wrapping_add(k2);
    bjmix(&mut a, &mut b, &mut c);
    let a2 = a.wrapping_add(k3);
    [
        bits_to_01(bjfinal(a2, b, c)),
        bits_to_01(bjfinal(a2, b.wrapping_add(1), c)),
        bits_to_01(bjfinal(a2, b.wrapping_add(2), c)),
    ]
}

// -- Internal helpers --------------------------------------------------------

pub(crate) const K0: u64 = 0xc3a5c85c97cb3127;
const K1: u64 = 0xb492b66fbe98f273;
pub(crate) const K2: u64 = 0x9ae16a3b2f90404f;

/// Read a little-endian u64 from `s` at position `pos` (const-safe).
#[inline]
const fn fetch64(s: &[u8], pos: usize) -> u64 {
    (s[pos] as u64)
        | ((s[pos + 1] as u64) << 8)
        | ((s[pos + 2] as u64) << 16)
        | ((s[pos + 3] as u64) << 24)
        | ((s[pos + 4] as u64) << 32)
        | ((s[pos + 5] as u64) << 40)
        | ((s[pos + 6] as u64) << 48)
        | ((s[pos + 7] as u64) << 56)
}

/// Read a little-endian u32 from `s` at position `pos` (const-safe).
#[inline]
const fn fetch32(s: &[u8], pos: usize) -> u32 {
    (s[pos] as u32)
        | ((s[pos + 1] as u32) << 8)
        | ((s[pos + 2] as u32) << 16)
        | ((s[pos + 3] as u32) << 24)
}

#[inline]
const fn rotate64(val: u64, shift: u32) -> u64 {
    if shift == 0 {
        val
    } else {
        val.rotate_right(shift)
    }
}

#[inline]
const fn shift_mix(val: u64) -> u64 {
    val ^ (val >> 47)
}

#[inline]
const fn hash_len_16(u: u64, v: u64, mul: u64) -> u64 {
    let a = (u ^ v).wrapping_mul(mul);
    let a = a ^ (a >> 47);
    let b = (v ^ a).wrapping_mul(mul);
    let b = b ^ (b >> 47);
    b.wrapping_mul(mul)
}

const fn hash_len_0_to_16(s: &[u8]) -> u64 {
    let len = s.len();
    if len >= 8 {
        let mul = K2.wrapping_add((len as u64).wrapping_mul(2));
        let a = fetch64(s, 0).wrapping_add(K2);
        let b = fetch64(s, len - 8);
        let c = rotate64(b, 37).wrapping_mul(mul).wrapping_add(a);
        let d = rotate64(a, 25).wrapping_add(b).wrapping_mul(mul);
        hash_len_16(c, d, mul)
    } else if len >= 4 {
        let mul = K2.wrapping_add((len as u64).wrapping_mul(2));
        let a = fetch32(s, 0) as u64;
        hash_len_16(
            (len as u64).wrapping_add(a << 3),
            fetch32(s, len - 4) as u64,
            mul,
        )
    } else if len > 0 {
        let a = s[0];
        let b = s[len >> 1];
        let c = s[len - 1];
        let y = (a as u32).wrapping_add((b as u32) << 8);
        let z = (len as u32).wrapping_add((c as u32) << 2);
        shift_mix((y as u64).wrapping_mul(K2) ^ (z as u64).wrapping_mul(K0)).wrapping_mul(K2)
    } else {
        K2
    }
}

const fn hash_len_17_to_32(s: &[u8]) -> u64 {
    let len = s.len();
    let mul = K2.wrapping_add((len as u64).wrapping_mul(2));
    let a = fetch64(s, 0).wrapping_mul(K1);
    let b = fetch64(s, 8);
    let c = fetch64(s, len - 8).wrapping_mul(mul);
    let d = fetch64(s, len - 16).wrapping_mul(K2);
    hash_len_16(
        rotate64(a.wrapping_add(b), 43)
            .wrapping_add(rotate64(c, 30))
            .wrapping_add(d),
        a.wrapping_add(rotate64(b.wrapping_add(K2), 18))
            .wrapping_add(c),
        mul,
    )
}

const fn hash_len_33_to_64(s: &[u8]) -> u64 {
    let len = s.len();
    let mul = K2.wrapping_add((len as u64).wrapping_mul(2));
    let a = fetch64(s, 0).wrapping_mul(K2);
    let b = fetch64(s, 8);
    let c = fetch64(s, len - 8).wrapping_mul(mul);
    let d = fetch64(s, len - 16).wrapping_mul(K2);
    let y = rotate64(a.wrapping_add(b), 43)
        .wrapping_add(rotate64(c, 30))
        .wrapping_add(d);
    let z = hash_len_16(
        y,
        a.wrapping_add(rotate64(b.wrapping_add(K2), 18))
            .wrapping_add(c),
        mul,
    );
    let e = fetch64(s, 16).wrapping_mul(mul);
    let f = fetch64(s, 24);
    let g = (y.wrapping_add(fetch64(s, len - 32))).wrapping_mul(mul);
    let h = (z.wrapping_add(fetch64(s, len - 24))).wrapping_mul(mul);
    hash_len_16(
        rotate64(e.wrapping_add(f), 43)
            .wrapping_add(rotate64(g, 30))
            .wrapping_add(h),
        e.wrapping_add(rotate64(f.wrapping_add(a), 18))
            .wrapping_add(g),
        mul,
    )
}

const fn weak_hash_len_32_with_seeds(s: &[u8], pos: usize, a: u64, b: u64) -> (u64, u64) {
    let w = fetch64(s, pos);
    let x = fetch64(s, pos + 8);
    let y = fetch64(s, pos + 16);
    let z = fetch64(s, pos + 24);
    let a = a.wrapping_add(w);
    let b = rotate64(b.wrapping_add(a).wrapping_add(z), 21);
    let c = a;
    let a = a.wrapping_add(x).wrapping_add(y);
    let b = b.wrapping_add(rotate64(a, 44));
    (a.wrapping_add(z), b.wrapping_add(c))
}

const fn hash_len_65_plus(s: &[u8]) -> u64 {
    let len = s.len();
    let seed: u64 = 81;

    let mut x = seed;
    let mut y = seed.wrapping_mul(K1).wrapping_add(113);
    let mut z = shift_mix(y.wrapping_mul(K2).wrapping_add(113)).wrapping_mul(K2);

    let mut v0: u64 = 0;
    let mut v1: u64 = 0;
    let mut w0: u64 = 0;
    let mut w1: u64 = 0;

    x = x.wrapping_mul(K2).wrapping_add(fetch64(s, 0));

    let end_idx = ((len - 1) / 64) * 64;
    let last64 = end_idx + ((len - 1) & 63) - 63;
    let mut pos: usize = 0;

    loop {
        x = rotate64(
            x.wrapping_add(y)
                .wrapping_add(v0)
                .wrapping_add(fetch64(s, pos + 8)),
            37,
        )
        .wrapping_mul(K1);
        y = rotate64(y.wrapping_add(v1).wrapping_add(fetch64(s, pos + 48)), 42).wrapping_mul(K1);
        x ^= w1;
        y = y.wrapping_add(v0).wrapping_add(fetch64(s, pos + 40));
        z = rotate64(z.wrapping_add(w0), 33).wrapping_mul(K1);

        let r1 = weak_hash_len_32_with_seeds(s, pos, v1.wrapping_mul(K1), x.wrapping_add(w0));
        v0 = r1.0;
        v1 = r1.1;

        let r2 = weak_hash_len_32_with_seeds(
            s,
            pos + 32,
            z.wrapping_add(w1),
            y.wrapping_add(fetch64(s, pos + 16)),
        );
        w0 = r2.0;
        w1 = r2.1;

        // swap z and x
        let tmp = z;
        z = x;
        x = tmp;

        pos += 64;
        if pos == end_idx {
            break;
        }
    }

    // Process the last 64 bytes.
    let mul = K1.wrapping_add((z & 0xff) << 1);
    let pos = last64;
    w0 = w0.wrapping_add(((len - 1) as u64) & 63);
    v0 = v0.wrapping_add(w0);
    w0 = w0.wrapping_add(v0);

    x = rotate64(
        x.wrapping_add(y)
            .wrapping_add(v0)
            .wrapping_add(fetch64(s, pos + 8)),
        37,
    )
    .wrapping_mul(mul);
    y = rotate64(y.wrapping_add(v1).wrapping_add(fetch64(s, pos + 48)), 42).wrapping_mul(mul);
    x ^= w1.wrapping_mul(9);
    y = y
        .wrapping_add(v0.wrapping_mul(9))
        .wrapping_add(fetch64(s, pos + 40));
    z = rotate64(z.wrapping_add(w0), 33).wrapping_mul(mul);

    let r1 = weak_hash_len_32_with_seeds(s, pos, v1.wrapping_mul(mul), x.wrapping_add(w0));
    v0 = r1.0;
    v1 = r1.1;

    let r2 = weak_hash_len_32_with_seeds(
        s,
        pos + 32,
        z.wrapping_add(w1),
        y.wrapping_add(fetch64(s, pos + 16)),
    );
    w0 = r2.0;
    w1 = r2.1;

    // swap z and x
    let tmp = z;
    z = x;
    x = tmp;

    hash_len_16(
        hash_len_16(v0, w0, mul)
            .wrapping_add(shift_mix(y).wrapping_mul(K0))
            .wrapping_add(z),
        hash_len_16(v1, w1, mul).wrapping_add(x),
        mul,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty() {
        assert_eq!(fingerprint64(b""), K2);
    }

    #[test]
    fn test_known_short() {
        let h1 = fingerprint64(b"a");
        let h2 = fingerprint64(b"a");
        assert_eq!(h1, h2);
        assert_ne!(h1, 0);
    }

    #[test]
    fn test_different_strings() {
        assert_ne!(fingerprint64(b"hello"), fingerprint64(b"world"));
        assert_ne!(fingerprint64(b"P"), fingerprint64(b"N"));
    }

    #[test]
    fn test_consistency() {
        let input = b"Open Shading Language";
        let h = fingerprint64(input);
        for _ in 0..100 {
            assert_eq!(fingerprint64(input), h);
        }
    }

    #[test]
    fn test_long_string() {
        // Test string > 64 bytes.
        let long: Vec<u8> = (0..256).map(|i| (i & 0xff) as u8).collect();
        let h = fingerprint64(&long);
        assert_ne!(h, 0);
        assert_eq!(fingerprint64(&long), h);
    }

    #[test]
    fn test_const_eval() {
        // Verify that the hash can be evaluated at compile time.
        const H: u64 = fingerprint64(b"test");
        assert_ne!(H, 0);
        assert_eq!(fingerprint64(b"test"), H);
    }

    #[test]
    fn test_oslhash() {
        let h = oslhash("hello");
        assert_ne!(h, 0);
        // Deterministic
        assert_eq!(oslhash("hello"), h);
        // Different strings -> different hashes
        assert_ne!(oslhash("hello"), oslhash("world"));
    }

    #[test]
    fn test_inthash_deterministic() {
        let h1 = inthash1(42);
        assert_eq!(inthash1(42), h1);
        assert_ne!(inthash1(42), inthash1(43));
    }

    #[test]
    fn test_inthash_dimensions() {
        // Each dimension count should produce different results
        let h1 = inthash1(1);
        let h2 = inthash2(1, 0);
        let h3 = inthash3(1, 0, 0);
        let h4 = inthash4(1, 0, 0, 0);
        let h5 = inthash5(1, 0, 0, 0, 0);
        // All should be different (different start_val due to length encoding)
        let hashes = [h1, h2, h3, h4, h5];
        for i in 0..hashes.len() {
            for j in (i + 1)..hashes.len() {
                assert_ne!(hashes[i], hashes[j], "hash dim {} == dim {}", i + 1, j + 1);
            }
        }
    }

    #[test]
    fn test_bits_to_01() {
        assert_eq!(bits_to_01(0), 0.0);
        // Max should be very close to 1.0
        let max_val = bits_to_01(u32::MAX);
        assert!((max_val - 1.0).abs() < 1e-6);
        // Mid value should be ~0.5
        let mid = bits_to_01(u32::MAX / 2);
        assert!((mid - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_bjfinal_avalanche() {
        // Changing one bit of input should change many bits of output
        let h1 = bjfinal(100, 200, 300);
        let h2 = bjfinal(101, 200, 300);
        assert_ne!(h1, h2);
        // Count differing bits (should be many for good avalanche)
        let diff_bits = (h1 ^ h2).count_ones();
        assert!(
            diff_bits > 4,
            "poor avalanche: only {} bits differ",
            diff_bits
        );
    }
}

// Bob Jenkins lookup3 hash (OIIO::bjhash) used by OSL's hash() builtin.
// Uses inthash1..4 defined above.

/// Reinterpret float bits as u32 (bitcast_to_uint)
#[inline]
fn f2u(x: f32) -> u32 {
    x.to_bits()
}

/// OSL hash(float) -> int
pub fn osl_hash_f(x: f32) -> i32 {
    inthash1(f2u(x)) as i32
}

/// OSL hash(float, float) -> int
pub fn osl_hash_ff(x: f32, y: f32) -> i32 {
    inthash2(f2u(x), f2u(y)) as i32
}

/// OSL hash(point/vector/normal) -> int
pub fn osl_hash_v(v: &[f32; 3]) -> i32 {
    inthash3(f2u(v[0]), f2u(v[1]), f2u(v[2])) as i32
}

/// OSL hash(point, float) -> int
pub fn osl_hash_vf(v: &[f32; 3], t: f32) -> i32 {
    inthash4(f2u(v[0]), f2u(v[1]), f2u(v[2]), f2u(t)) as i32
}
