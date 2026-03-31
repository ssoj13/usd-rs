/// Integration tests for hash.rs - port of testenv/hash.cpp
///
/// Tests hash consistency, combine semantics, and avalanche properties
/// for TfHash and related utilities.
///
/// Note: f32/f64 do not implement std::hash::Hash. Tests that involve floats
/// hash via their bit representation (to_bits()), matching the spirit of the
/// C++ TfHash which treats float bytes uniformly.
use std::collections::HashMap;
use usd_tf::hash::{TfHash, TfHasher, combine_two, hash_bytes, hash_combine, hash_str};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Count how many output bits flip when a single input bit is flipped.
fn avalanche_one(x: u64, counts: &mut [u32; 64]) {
    let x_hash = hash_value(x);
    for i in 0..64u32 {
        let x_prime = x ^ (1u64 << i);
        let x_prime_hash = hash_value(x_prime);
        let flips = x_hash ^ x_prime_hash;
        for (index, count) in counts.iter_mut().enumerate() {
            if (flips >> index) & 1 == 1 {
                *count += 1;
            }
        }
    }
}

fn hash_value(x: u64) -> u64 {
    use std::hash::Hasher;
    let mut h = TfHasher::new();
    h.write_u64(x);
    h.finish()
}

// ---------------------------------------------------------------------------
// Basic consistency
// ---------------------------------------------------------------------------

#[test]
fn same_value_same_hash() {
    let h1 = TfHash::hash(&42u64);
    let h2 = TfHash::hash(&42u64);
    assert_eq!(h1, h2, "same value must produce same hash");
}

#[test]
fn different_integers_different_hashes() {
    let h1 = TfHash::hash(&1u64);
    let h2 = TfHash::hash(&2u64);
    assert_ne!(h1, h2);
}

#[test]
fn string_consistency() {
    let s = "hello world";
    let h1 = TfHash::hash(&s);
    let h2 = TfHash::hash(&s);
    assert_eq!(h1, h2);
}

#[test]
fn different_strings_different_hashes() {
    assert_ne!(TfHash::hash(&"hello"), TfHash::hash(&"world"));
}

#[test]
fn bytes_match_str() {
    let h1 = hash_str("hello world");
    let h2 = hash_bytes(b"hello world");
    assert_eq!(h1, h2, "hash_str and hash_bytes must agree");
}

// ---------------------------------------------------------------------------
// Enum-like values (C++ tests FooEnum/BarEnum variants)
// ---------------------------------------------------------------------------

#[test]
fn enum_like_u8_hashes_distinct() {
    let ha = TfHash::hash(&0u8);
    let hb = TfHash::hash(&1u8);
    let hc = TfHash::hash(&2u8);
    assert_ne!(ha, hb);
    assert_ne!(hb, hc);
    assert_ne!(ha, hc);
}

#[test]
fn enum_like_i32_hashes_distinct() {
    // Mirrors: hash(FooA), hash(FooB), hash(FooC)
    let ha = TfHash::hash(&0i32);
    let hb = TfHash::hash(&1i32);
    let hc = TfHash::hash(&2i32);
    assert_ne!(ha, hb);
    assert_ne!(hb, hc);
    assert_ne!(ha, hc);
}

// ---------------------------------------------------------------------------
// Collections
// ---------------------------------------------------------------------------

#[test]
fn vec_hash_consistency() {
    let v = vec![1i32, 2, 3, 4, 5];
    let h1 = TfHash::hash(&v);
    let h2 = TfHash::hash(&v);
    assert_eq!(h1, h2);
}

#[test]
fn vec_order_matters() {
    let v1 = vec![1i32, 2, 3];
    let v2 = vec![3i32, 2, 1];
    assert_ne!(TfHash::hash(&v1), TfHash::hash(&v2));
}

#[test]
fn vec_bool_consistency() {
    let v = vec![true, false, true];
    let h1 = TfHash::hash(&v);
    let h2 = TfHash::hash(&v);
    assert_eq!(h1, h2);
}

// ---------------------------------------------------------------------------
// Pairs and tuples - f32/f64 hashed via to_bits() since f32 !Hash in Rust
// ---------------------------------------------------------------------------

fn hash_int_float_pair(i: i32, f: f32) -> u64 {
    // Concatenate bytes of both values to produce a combined hash,
    // matching the C++ (int, float) pair hash test.
    let mut buf = [0u8; 8];
    buf[..4].copy_from_slice(&i.to_le_bytes());
    buf[4..].copy_from_slice(&f.to_bits().to_le_bytes());
    hash_bytes(&buf)
}

fn hash_int_float_double_triple(i: i32, f: f32, d: f64) -> u64 {
    let mut buf = [0u8; 16];
    buf[..4].copy_from_slice(&i.to_le_bytes());
    buf[4..8].copy_from_slice(&f.to_bits().to_le_bytes());
    buf[8..].copy_from_slice(&d.to_bits().to_le_bytes());
    hash_bytes(&buf)
}

#[test]
fn pair_int_float_hash_consistency() {
    // Mirrors: hash(pair<int, float>{1, 2.34})
    let h1 = hash_int_float_pair(1, 2.34);
    let h2 = hash_int_float_pair(1, 2.34);
    assert_eq!(h1, h2);
}

#[test]
fn tuple_2_hash_consistency() {
    let h1 = hash_int_float_pair(1, 2.34);
    let h2 = hash_int_float_pair(1, 2.34);
    assert_eq!(h1, h2);
}

#[test]
fn tuple_3_hash_consistency() {
    let h1 = hash_int_float_double_triple(1, 2.34, 5.678);
    let h2 = hash_int_float_double_triple(1, 2.34, 5.678);
    assert_eq!(h1, h2);
}

// ---------------------------------------------------------------------------
// Float -0.0 / 0.0: C++ test prints but does not assert equality.
// We verify bit-level consistency only.
// ---------------------------------------------------------------------------

#[test]
fn float_zero_bit_hash_consistent() {
    let h1 = hash_bytes(&0.0f32.to_bits().to_le_bytes());
    let h2 = hash_bytes(&0.0f32.to_bits().to_le_bytes());
    assert_eq!(h1, h2);

    let h3 = hash_bytes(&(-0.0f32).to_bits().to_le_bytes());
    let h4 = hash_bytes(&(-0.0f32).to_bits().to_le_bytes());
    assert_eq!(h3, h4);
}

#[test]
fn double_zero_bit_hash_consistent() {
    let h1 = hash_bytes(&0.0f64.to_bits().to_le_bytes());
    let h2 = hash_bytes(&0.0f64.to_bits().to_le_bytes());
    assert_eq!(h1, h2);
}

// ---------------------------------------------------------------------------
// optional / variant equivalents
// ---------------------------------------------------------------------------

#[test]
fn optional_some_consistent() {
    let a: Option<String> = Some("xyz".to_string());
    let b: Option<String> = Some("xyz".to_string());
    assert_eq!(TfHash::hash(&a), TfHash::hash(&b));
}

#[test]
fn optional_none_consistent() {
    let a: Option<String> = None;
    let b: Option<String> = None;
    assert_eq!(TfHash::hash(&a), TfHash::hash(&b));
}

#[test]
fn optional_some_ne_none() {
    let a: Option<i32> = Some(1);
    let b: Option<i32> = None;
    assert_ne!(TfHash::hash(&a), TfHash::hash(&b));
}

// ---------------------------------------------------------------------------
// combine_two / hash_combine
// ---------------------------------------------------------------------------

#[test]
fn combine_two_not_commutative() {
    // C++ TfHash::Combine is order-sensitive
    let c1 = combine_two(100, 200);
    let c2 = combine_two(200, 100);
    assert_ne!(c1, c2);
}

#[test]
fn combine_two_deterministic() {
    assert_eq!(combine_two(100, 200), combine_two(100, 200));
}

#[test]
fn hash_combine_order_matters() {
    let a = hash_combine(&[1, 2, 3]);
    let b = hash_combine(&[3, 2, 1]);
    assert_ne!(a, b);
}

#[test]
fn hash_combine_mirrors_cpp_combine() {
    // C++: TfHash::Combine(vint, intfloat, vp)
    // Deterministic across two calls.
    let v_int_hash = TfHash::hash(&vec![1i32, 2, 3, 4, 5]);
    let pair_hash = hash_int_float_pair(1, 2.34_f32);
    let combined1 = TfHash::combine(&[v_int_hash, pair_hash]);
    let combined2 = TfHash::combine(&[v_int_hash, pair_hash]);
    assert_eq!(combined1, combined2);
}

// ---------------------------------------------------------------------------
// BuildHasher usage with std::collections::HashMap
// ---------------------------------------------------------------------------

#[test]
fn tf_hash_as_build_hasher() {
    let mut map: HashMap<String, i32, TfHash> = HashMap::with_hasher(TfHash);
    map.insert("one".to_string(), 1);
    map.insert("two".to_string(), 2);
    assert_eq!(map.get("one"), Some(&1));
    assert_eq!(map.get("two"), Some(&2));
    assert_eq!(map.get("three"), None);
}

// ---------------------------------------------------------------------------
// Avalanche / bit-distribution (mirrors _TestStatsOne / _TestStatsTwo)
// ---------------------------------------------------------------------------

#[test]
fn avalanche_each_output_bit_flips() {
    const N: u64 = 1000;
    let mut counts = [0u32; 64];
    for i in 0..N {
        let x = i.wrapping_shl(5);
        avalanche_one(x, &mut counts);
    }
    for (bit, &count) in counts.iter().enumerate() {
        assert!(
            count > 0,
            "output bit {bit} never flipped — hash has poor avalanche"
        );
    }
}

// ---------------------------------------------------------------------------
// TfHasher seed
// ---------------------------------------------------------------------------

#[test]
fn hasher_seed_affects_output() {
    use std::hash::Hasher;
    let mut h1 = TfHasher::new();
    h1.write(b"hello");
    let r1 = h1.finish();

    let mut h2 = TfHasher::with_seed(42);
    h2.write(b"hello");
    let r2 = h2.finish();

    assert_ne!(r1, r2);
}

#[test]
fn hasher_same_seed_deterministic() {
    use std::hash::Hasher;
    let mut h1 = TfHasher::with_seed(99);
    h1.write(b"data");
    let r1 = h1.finish();

    let mut h2 = TfHasher::with_seed(99);
    h2.write(b"data");
    let r2 = h2.finish();

    assert_eq!(r1, r2);
}

// ---------------------------------------------------------------------------
// Integers at various orders of magnitude (C++ for-loop over orders 10..1M)
// ---------------------------------------------------------------------------

#[test]
fn integer_hash_all_magnitudes_consistent() {
    let mut order = 10i32;
    while order < 1_000_000 {
        let step = order / 10;
        let mut i = 0;
        while i < order {
            let h1 = TfHash::hash(&i);
            let h2 = TfHash::hash(&i);
            assert_eq!(h1, h2, "hash not consistent for i={i}");
            i += step;
        }
        order *= 10;
    }
}

// ---------------------------------------------------------------------------
// Two-word struct (mirrors struct Two + TestTwo)
// ---------------------------------------------------------------------------

#[derive(Clone, Copy)]
struct Two {
    x: u32,
    y: u32,
}

fn hash_two(t: Two) -> u64 {
    let mut buf = [0u8; 8];
    buf[..4].copy_from_slice(&t.x.to_le_bytes());
    buf[4..].copy_from_slice(&t.y.to_le_bytes());
    hash_bytes(&buf)
}

fn avalanche_two(t: Two, counts: &mut [u32; 64]) {
    let t_hash = hash_two(t);
    for i in 0..32u32 {
        let t_prime = Two {
            x: t.x ^ (1 << i),
            y: t.y,
        };
        let flips = t_hash ^ hash_two(t_prime);
        for (index, count) in counts.iter_mut().enumerate() {
            if (flips >> index) & 1 == 1 {
                *count += 1;
            }
        }
    }
    for i in 0..32u32 {
        let t_prime = Two {
            x: t.x,
            y: t.y ^ (1 << i),
        };
        let flips = t_hash ^ hash_two(t_prime);
        for (index, count) in counts.iter_mut().enumerate() {
            if (flips >> index) & 1 == 1 {
                *count += 1;
            }
        }
    }
}

#[test]
fn two_word_struct_avalanche() {
    const N: u64 = 1000;
    let mut counts = [0u32; 64];
    for i in 0..N {
        let t = Two {
            x: (i.wrapping_shl(5)) as u32,
            y: (i.wrapping_shr(5)) as u32,
        };
        avalanche_two(t, &mut counts);
    }
    for (bit, &count) in counts.iter().enumerate() {
        assert!(
            count > 0,
            "Two: output bit {bit} never flipped — poor avalanche"
        );
    }
}
