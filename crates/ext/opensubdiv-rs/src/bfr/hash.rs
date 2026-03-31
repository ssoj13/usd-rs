//! SpookyHash-based 32/64-bit hash functions for BFR caching.
//!
//! Ported from OpenSubdiv bfr/hash.h/.cpp (Bob Jenkins SpookyHash v2, public domain).

/// Compute a 32-bit hash of `data` with seed 0.
///
/// Part of the C++ API parity (C++ exposes both 32- and 64-bit variants).
/// Currently used in tests only; the 64-bit variant is used for cache keys.
#[allow(dead_code)]
pub(crate) fn hash32(data: &[u8]) -> u32 {
    hash32_seeded(data, 0)
}

/// Compute a 32-bit hash of `data` with an explicit seed.
#[allow(dead_code)]
pub(crate) fn hash32_seeded(data: &[u8], seed: u32) -> u32 {
    let (h1, _) = spooky_hash128(data, seed as u64, seed as u64);
    h1 as u32
}

/// Compute a 64-bit hash of `data` with seed 0.
pub(crate) fn hash64(data: &[u8]) -> u64 {
    hash64_seeded(data, 0)
}

/// Compute a 64-bit hash of `data` with an explicit seed.
pub(crate) fn hash64_seeded(data: &[u8], seed: u64) -> u64 {
    let (h1, _) = spooky_hash128(data, seed, seed);
    h1
}

// ---------------------------------------------------------------------------
// SpookyHash v2 internals (Bob Jenkins, public domain)
// ---------------------------------------------------------------------------

const SC_CONST: u64 = 0xdeadbeef_deadbeef_u64;
const SC_NUM_VARS: usize = 12;
const SC_BLOCK_SIZE: usize = SC_NUM_VARS * 8; // 96 bytes
const SC_BUF_SIZE: usize   = 2 * SC_BLOCK_SIZE; // 192 bytes

#[inline(always)]
fn rot64(x: u64, k: u32) -> u64 { x.rotate_left(k) }

#[allow(clippy::too_many_arguments)]
#[inline(always)]
fn mix(
    d: &[u64; 12],
    s0: &mut u64, s1: &mut u64, s2:  &mut u64, s3:  &mut u64,
    s4: &mut u64, s5: &mut u64, s6:  &mut u64, s7:  &mut u64,
    s8: &mut u64, s9: &mut u64, s10: &mut u64, s11: &mut u64,
) {
    *s0  = s0.wrapping_add(d[0]);  *s2  ^= *s10; *s11 ^= *s0;  *s0  = rot64(*s0,11);  *s11 = s11.wrapping_add(*s1);
    *s1  = s1.wrapping_add(d[1]);  *s3  ^= *s11; *s0  ^= *s1;  *s1  = rot64(*s1,32);  *s0  = s0.wrapping_add(*s2);
    *s2  = s2.wrapping_add(d[2]);  *s4  ^= *s0;  *s1  ^= *s2;  *s2  = rot64(*s2,43);  *s1  = s1.wrapping_add(*s3);
    *s3  = s3.wrapping_add(d[3]);  *s5  ^= *s1;  *s2  ^= *s3;  *s3  = rot64(*s3,31);  *s2  = s2.wrapping_add(*s4);
    *s4  = s4.wrapping_add(d[4]);  *s6  ^= *s2;  *s3  ^= *s4;  *s4  = rot64(*s4,17);  *s3  = s3.wrapping_add(*s5);
    *s5  = s5.wrapping_add(d[5]);  *s7  ^= *s3;  *s4  ^= *s5;  *s5  = rot64(*s5,28);  *s4  = s4.wrapping_add(*s6);
    *s6  = s6.wrapping_add(d[6]);  *s8  ^= *s4;  *s5  ^= *s6;  *s6  = rot64(*s6,39);  *s5  = s5.wrapping_add(*s7);
    *s7  = s7.wrapping_add(d[7]);  *s9  ^= *s5;  *s6  ^= *s7;  *s7  = rot64(*s7,57);  *s6  = s6.wrapping_add(*s8);
    *s8  = s8.wrapping_add(d[8]);  *s10 ^= *s6;  *s7  ^= *s8;  *s8  = rot64(*s8,55);  *s7  = s7.wrapping_add(*s9);
    *s9  = s9.wrapping_add(d[9]);  *s11 ^= *s7;  *s8  ^= *s9;  *s9  = rot64(*s9,54);  *s8  = s8.wrapping_add(*s10);
    *s10 = s10.wrapping_add(d[10]);*s0  ^= *s8;  *s9  ^= *s10; *s10 = rot64(*s10,22); *s9  = s9.wrapping_add(*s11);
    *s11 = s11.wrapping_add(d[11]);*s1  ^= *s9;  *s10 ^= *s11; *s11 = rot64(*s11,46); *s10 = s10.wrapping_add(*s0);
}

#[allow(clippy::too_many_arguments)]
#[inline(always)]
fn end_partial(
    h0: &mut u64, h1: &mut u64, h2:  &mut u64, h3:  &mut u64,
    h4: &mut u64, h5: &mut u64, h6:  &mut u64, h7:  &mut u64,
    h8: &mut u64, h9: &mut u64, h10: &mut u64, h11: &mut u64,
) {
    *h11 = h11.wrapping_add(*h1);  *h2  ^= *h11; *h1  = rot64(*h1,44);
    *h0  = h0.wrapping_add(*h2);   *h3  ^= *h0;  *h2  = rot64(*h2,15);
    *h1  = h1.wrapping_add(*h3);   *h4  ^= *h1;  *h3  = rot64(*h3,34);
    *h2  = h2.wrapping_add(*h4);   *h5  ^= *h2;  *h4  = rot64(*h4,21);
    *h3  = h3.wrapping_add(*h5);   *h6  ^= *h3;  *h5  = rot64(*h5,38);
    *h4  = h4.wrapping_add(*h6);   *h7  ^= *h4;  *h6  = rot64(*h6,33);
    *h5  = h5.wrapping_add(*h7);   *h8  ^= *h5;  *h7  = rot64(*h7,10);
    *h6  = h6.wrapping_add(*h8);   *h9  ^= *h6;  *h8  = rot64(*h8,13);
    *h7  = h7.wrapping_add(*h9);   *h10 ^= *h7;  *h9  = rot64(*h9,38);
    *h8  = h8.wrapping_add(*h10);  *h11 ^= *h8;  *h10 = rot64(*h10,53);
    *h9  = h9.wrapping_add(*h11);  *h0  ^= *h9;  *h11 = rot64(*h11,42);
    *h10 = h10.wrapping_add(*h0);  *h1  ^= *h10; *h0  = rot64(*h0,54);
}

#[allow(clippy::too_many_arguments)]
#[inline(always)]
fn end(
    d: &[u64; 12],
    h0: &mut u64, h1: &mut u64, h2:  &mut u64, h3:  &mut u64,
    h4: &mut u64, h5: &mut u64, h6:  &mut u64, h7:  &mut u64,
    h8: &mut u64, h9: &mut u64, h10: &mut u64, h11: &mut u64,
) {
    *h0  = h0.wrapping_add(d[0]);  *h1  = h1.wrapping_add(d[1]);
    *h2  = h2.wrapping_add(d[2]);  *h3  = h3.wrapping_add(d[3]);
    *h4  = h4.wrapping_add(d[4]);  *h5  = h5.wrapping_add(d[5]);
    *h6  = h6.wrapping_add(d[6]);  *h7  = h7.wrapping_add(d[7]);
    *h8  = h8.wrapping_add(d[8]);  *h9  = h9.wrapping_add(d[9]);
    *h10 = h10.wrapping_add(d[10]); *h11 = h11.wrapping_add(d[11]);
    end_partial(h0, h1, h2, h3, h4, h5, h6, h7, h8, h9, h10, h11);
    end_partial(h0, h1, h2, h3, h4, h5, h6, h7, h8, h9, h10, h11);
    end_partial(h0, h1, h2, h3, h4, h5, h6, h7, h8, h9, h10, h11);
}

#[inline(always)]
fn short_mix(h0: &mut u64, h1: &mut u64, h2: &mut u64, h3: &mut u64) {
    *h2 = rot64(*h2,50); *h2 = h2.wrapping_add(*h3); *h0 ^= *h2;
    *h3 = rot64(*h3,52); *h3 = h3.wrapping_add(*h0); *h1 ^= *h3;
    *h0 = rot64(*h0,30); *h0 = h0.wrapping_add(*h1); *h2 ^= *h0;
    *h1 = rot64(*h1,41); *h1 = h1.wrapping_add(*h2); *h3 ^= *h1;
    *h2 = rot64(*h2,54); *h2 = h2.wrapping_add(*h3); *h0 ^= *h2;
    *h3 = rot64(*h3,48); *h3 = h3.wrapping_add(*h0); *h1 ^= *h3;
    *h0 = rot64(*h0,38); *h0 = h0.wrapping_add(*h1); *h2 ^= *h0;
    *h1 = rot64(*h1,37); *h1 = h1.wrapping_add(*h2); *h3 ^= *h1;
    *h2 = rot64(*h2,62); *h2 = h2.wrapping_add(*h3); *h0 ^= *h2;
    *h3 = rot64(*h3,34); *h3 = h3.wrapping_add(*h0); *h1 ^= *h3;
    *h0 = rot64(*h0,5);  *h0 = h0.wrapping_add(*h1); *h2 ^= *h0;
    *h1 = rot64(*h1,36); *h1 = h1.wrapping_add(*h2); *h3 ^= *h1;
}

#[inline(always)]
fn short_end(h0: &mut u64, h1: &mut u64, h2: &mut u64, h3: &mut u64) {
    *h3 ^= *h2; *h2 = rot64(*h2,15); *h3 = h3.wrapping_add(*h2);
    *h0 ^= *h3; *h3 = rot64(*h3,52); *h0 = h0.wrapping_add(*h3);
    *h1 ^= *h0; *h0 = rot64(*h0,26); *h1 = h1.wrapping_add(*h0);
    *h2 ^= *h1; *h1 = rot64(*h1,51); *h2 = h2.wrapping_add(*h1);
    *h3 ^= *h2; *h2 = rot64(*h2,28); *h3 = h3.wrapping_add(*h2);
    *h0 ^= *h3; *h3 = rot64(*h3,9);  *h0 = h0.wrapping_add(*h3);
    *h1 ^= *h0; *h0 = rot64(*h0,47); *h1 = h1.wrapping_add(*h0);
    *h2 ^= *h1; *h1 = rot64(*h1,54); *h2 = h2.wrapping_add(*h1);
    *h3 ^= *h2; *h2 = rot64(*h2,32); *h3 = h3.wrapping_add(*h2);
    *h0 ^= *h3; *h3 = rot64(*h3,25); *h0 = h0.wrapping_add(*h3);
    *h1 ^= *h0; *h0 = rot64(*h0,63); *h1 = h1.wrapping_add(*h0);
}

/// Read a u64 from a byte slice at byte offset `off` (little-endian, safe).
#[inline(always)]
fn read_u64(bytes: &[u8], off: usize) -> u64 {
    let end = (off + 8).min(bytes.len());
    let mut buf = [0u8; 8];
    buf[..end - off].copy_from_slice(&bytes[off..end]);
    u64::from_le_bytes(buf)
}

#[inline(always)]
fn read_u32(bytes: &[u8], off: usize) -> u64 {
    let end = (off + 4).min(bytes.len());
    let mut buf = [0u8; 4];
    buf[..end - off].copy_from_slice(&bytes[off..end]);
    u32::from_le_bytes(buf) as u64
}

/// Short-message path (< 192 bytes).
fn spooky_short(message: &[u8], hash1: &mut u64, hash2: &mut u64) {
    let length = message.len();
    let remainder = length % 32;
    let mut a = *hash1;
    let mut b = *hash2;
    let mut c = SC_CONST;
    let mut d = SC_CONST;

    // Process complete 32-byte blocks
    let num_full = length / 32;
    for i in 0..num_full {
        let base = i * 32;
        c = c.wrapping_add(read_u64(message, base));
        d = d.wrapping_add(read_u64(message, base + 8));
        short_mix(&mut a, &mut b, &mut c, &mut d);
        a = a.wrapping_add(read_u64(message, base + 16));
        b = b.wrapping_add(read_u64(message, base + 24));
    }

    let tail_base = num_full * 32;
    let rem16 = remainder % 16; // remainder within the last 16-byte half

    // Handle 16+ remaining bytes if present
    if remainder >= 16 {
        c = c.wrapping_add(read_u64(message, tail_base));
        d = d.wrapping_add(read_u64(message, tail_base + 8));
        short_mix(&mut a, &mut b, &mut c, &mut d);
    }

    let tail = tail_base + if remainder >= 16 { 16 } else { 0 };

    // Encode length in high byte of d
    d = d.wrapping_add((length as u64) << 56);

    // Handle last 0..15 bytes
    let r = rem16;
    if r >= 15 { d = d.wrapping_add(read_u64(message, tail + 8) & 0x00ff_ffff_ffff_ffff_u64); }
    else if r >= 14 { d = d.wrapping_add(read_u64(message, tail + 8) & 0x0000_ffff_ffff_ffff_u64); }
    else if r >= 13 { d = d.wrapping_add(read_u64(message, tail + 8) & 0x0000_00ff_ffff_ffff_u64); }
    else if r >= 12 { d = d.wrapping_add(read_u32(message, tail + 8)); c = c.wrapping_add(read_u64(message, tail)); }
    else if r >= 11 { d = d.wrapping_add(read_u64(message, tail + 8) & 0x0000_0000_00ff_ffff_u64); c = c.wrapping_add(read_u64(message, tail)); }
    else if r >= 10 { d = d.wrapping_add(read_u64(message, tail + 8) & 0x0000_0000_0000_ffff_u64); c = c.wrapping_add(read_u64(message, tail)); }
    else if r >= 9  { d = d.wrapping_add(read_u64(message, tail + 8) & 0x0000_0000_0000_00ff_u64); c = c.wrapping_add(read_u64(message, tail)); }
    else if r >= 8  { c = c.wrapping_add(read_u64(message, tail)); }
    else if r >= 7  { c = c.wrapping_add(read_u64(message, tail) & 0x00ff_ffff_ffff_ffff_u64); }
    else if r >= 6  { c = c.wrapping_add(read_u64(message, tail) & 0x0000_ffff_ffff_ffff_u64); }
    else if r >= 5  { c = c.wrapping_add(read_u64(message, tail) & 0x0000_00ff_ffff_ffff_u64); }
    else if r >= 4  { c = c.wrapping_add(read_u32(message, tail)); }
    else if r >= 3  { c = c.wrapping_add(read_u64(message, tail) & 0x0000_0000_00ff_ffff_u64); }
    else if r >= 2  { c = c.wrapping_add(read_u64(message, tail) & 0x0000_0000_0000_ffff_u64); }
    else if r >= 1  { c = c.wrapping_add(read_u64(message, tail) & 0x0000_0000_0000_00ff_u64); }
    else            { c = c.wrapping_add(SC_CONST); d = d.wrapping_add(SC_CONST); }

    short_end(&mut a, &mut b, &mut c, &mut d);
    *hash1 = a;
    *hash2 = b;
}

/// Load 12 u64s from a 96-byte aligned block.
#[inline(always)]
fn load_block(message: &[u8], block_offset: usize) -> [u64; 12] {
    let mut d = [0u64; 12];
    for (i, v) in d.iter_mut().enumerate() {
        *v = read_u64(message, block_offset + i * 8);
    }
    d
}

/// Full SpookyHash128. Returns (hash1, hash2).
fn spooky_hash128(message: &[u8], seed1: u64, seed2: u64) -> (u64, u64) {
    let mut hash1 = seed1;
    let mut hash2 = seed2;

    if message.len() < SC_BUF_SIZE {
        spooky_short(message, &mut hash1, &mut hash2);
        return (hash1, hash2);
    }

    let (mut h0, mut h3, mut h6, mut h9)  = (hash1, hash1, hash1, hash1);
    let (mut h1, mut h4, mut h7, mut h10) = (hash2, hash2, hash2, hash2);
    let (mut h2, mut h5, mut h8, mut h11) = (SC_CONST, SC_CONST, SC_CONST, SC_CONST);

    let num_blocks = message.len() / SC_BLOCK_SIZE;
    for block in 0..num_blocks {
        let d = load_block(message, block * SC_BLOCK_SIZE);
        mix(&d, &mut h0, &mut h1, &mut h2, &mut h3,
            &mut h4, &mut h5, &mut h6, &mut h7,
            &mut h8, &mut h9, &mut h10, &mut h11);
    }

    // Handle the last partial block (padded with zeros, length in last byte)
    let remainder = message.len() - num_blocks * SC_BLOCK_SIZE;
    let mut buf = [0u8; SC_BLOCK_SIZE];
    let tail_start = num_blocks * SC_BLOCK_SIZE;
    buf[..remainder].copy_from_slice(&message[tail_start..tail_start + remainder]);
    buf[SC_BLOCK_SIZE - 1] = remainder as u8;

    let d = {
        let mut d = [0u64; 12];
        for (i, v) in d.iter_mut().enumerate() {
            let off = i * 8;
            v.copy_from(&buf[off..off + 8]);
        }
        d
    };

    end(&d, &mut h0, &mut h1, &mut h2, &mut h3,
        &mut h4, &mut h5, &mut h6, &mut h7,
        &mut h8, &mut h9, &mut h10, &mut h11);

    (h0, h1)
}

/// Helper to copy bytes into a u64 (little-endian).
trait CopyFromSlice {
    fn copy_from(&mut self, s: &[u8]);
}
impl CopyFromSlice for u64 {
    #[inline(always)]
    fn copy_from(&mut self, s: &[u8]) {
        let mut buf = [0u8; 8];
        let n = s.len().min(8);
        buf[..n].copy_from_slice(&s[..n]);
        *self = u64::from_le_bytes(buf);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash64_empty_deterministic() {
        assert_eq!(hash64(b""), hash64(b""));
    }

    #[test]
    fn hash64_different_inputs() {
        assert_ne!(hash64(b"hello"), hash64(b"world"));
    }

    #[test]
    fn hash64_seed_changes_result() {
        let a = hash64(b"test");
        let b = hash64_seeded(b"test", 42);
        assert_ne!(a, b);
    }

    #[test]
    fn hash32_consistent() {
        let v = hash32(b"OpenSubdiv");
        assert_eq!(v, hash32(b"OpenSubdiv"));
        assert_ne!(v, hash32(b"opensubdiv"));
    }

    #[test]
    fn hash64_long_path() {
        // > 192 bytes hits full Hash128 path
        let data = vec![0xABu8; 256];
        let h = hash64(&data);
        assert_ne!(h, 0);
        assert_eq!(h, hash64(&data));
    }

    #[test]
    fn hash64_medium_path() {
        // Short path: < 192 bytes
        let data = vec![0x12u8; 64];
        let h = hash64(&data);
        assert_ne!(h, 0);
        assert_eq!(h, hash64(&data));
    }
}
