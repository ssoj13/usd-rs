// Port of pxr/base/tf/testenv/compressedBits.cpp

use usd_tf::bits::Bits;
use usd_tf::compressed_bits::CompressedBits;

// Verifies that Bits and CompressedBits APIs return equal values.
fn verify_equality(a: &Bits, b: &CompressedBits) {
    assert_eq!(a.get_size(), b.size());
    assert_eq!(a.get_first_set(), b.get_first_set());
    assert_eq!(a.get_last_set(), b.get_last_set());
    assert_eq!(a.get_num_set(), b.get_num_set());
    assert_eq!(a.are_all_set(), b.are_all_set());
    assert_eq!(a.are_all_unset(), b.are_all_unset());
    assert_eq!(a.is_any_set(), b.is_any_set());
    assert_eq!(a.is_any_unset(), b.is_any_unset());
    assert_eq!(a.are_contiguously_set(), b.are_contiguously_set());
    assert_eq!(a.as_string_left_to_right(), b.as_string_left_to_right());

    for i in 0..a.get_size() {
        assert_eq!(a.is_set(i), b.is_set(i));
    }
}

// Verifies equality and also checks round-trip conversions.
fn verify_equality_with_conversion(a: &Bits, b: &CompressedBits) {
    verify_equality(a, b);

    let c = CompressedBits::from(&*a as &Bits);
    verify_equality(a, &c);

    let d = b.to_bits();
    verify_equality(&d, b);
}

#[test]
fn test_basic_api() {
    let b = CompressedBits::new(4);

    assert_eq!(b.size(), 4);
    assert_eq!(b.get_num_set(), 0);
    assert!(!b.are_all_set());
    assert!(b.are_all_unset());
    assert!(!b.is_any_set());
    assert!(b.is_any_unset());
    assert!(!b.are_contiguously_set());
    assert_eq!(b.get_first_set(), b.size());
    assert_eq!(b.get_last_set(), b.size());
}

#[test]
fn test_set_single_bit() {
    let mut b = CompressedBits::new(4);
    b.set(0);

    assert!(b.is_set(0));
    assert_eq!(b.size(), 4);
    assert_eq!(b.get_num_set(), 1);
    assert!(!b.are_all_set());
    assert!(!b.are_all_unset());
    assert!(b.is_any_set());
    assert!(b.is_any_unset());
    assert!(b.are_contiguously_set());
    assert_eq!(b.get_first_set(), 0);
    assert_eq!(b.get_last_set(), 0);
    assert_eq!(b.as_string_left_to_right(), "1000");
    assert_eq!(b.as_string_right_to_left(), "0001");
}

#[test]
fn test_set_second_bit() {
    let mut b = CompressedBits::new(4);
    b.set(0);
    b.set(2);

    assert!(b.is_set(0));
    assert!(b.is_set(2));
    assert_eq!(b.size(), 4);
    assert_eq!(b.get_num_set(), 2);
    assert!(!b.are_all_set());
    assert!(!b.are_all_unset());
    assert!(b.is_any_set());
    assert!(b.is_any_unset());
    assert!(!b.are_contiguously_set());
    assert_eq!(b.get_first_set(), 0);
    assert_eq!(b.get_last_set(), 2);
    assert_eq!(b.as_string_left_to_right(), "1010");
    assert_eq!(b.as_string_right_to_left(), "0101");
}

#[test]
fn test_assign_third_bit() {
    let mut b = CompressedBits::new(4);
    b.set(0);
    b.set(2);
    b.assign(1, true);

    assert!(b.is_set(0));
    assert!(b.is_set(1));
    assert!(b.is_set(2));
    assert_eq!(b.size(), 4);
    assert_eq!(b.get_num_set(), 3);
    assert!(!b.are_all_set());
    assert!(!b.are_all_unset());
    assert!(b.is_any_set());
    assert!(b.is_any_unset());
    assert!(b.are_contiguously_set());
    assert_eq!(b.get_first_set(), 0);
    assert_eq!(b.get_last_set(), 2);
    assert_eq!(b.as_string_left_to_right(), "1110");
    assert_eq!(b.as_string_right_to_left(), "0111");
}

#[test]
fn test_set_all() {
    let mut b = CompressedBits::new(4);
    b.set(0);
    b.set(2);
    b.assign(1, true);
    b.set_all();

    assert!(b.is_set(0));
    assert!(b.is_set(1));
    assert!(b.is_set(2));
    assert!(b.is_set(3));
    assert_eq!(b.size(), 4);
    assert_eq!(b.get_num_set(), 4);
    assert!(b.are_all_set());
    assert!(!b.are_all_unset());
    assert!(b.is_any_set());
    assert!(!b.is_any_unset());
    assert!(b.are_contiguously_set());
    assert_eq!(b.get_first_set(), 0);
    assert_eq!(b.get_last_set(), 3);
    assert_eq!(b.as_string_left_to_right(), "1111");
    assert_eq!(b.as_string_right_to_left(), "1111");
}

#[test]
fn test_unset_bit_via_assign() {
    let mut b = CompressedBits::new(4);
    b.set_all();
    b.assign(0, false);

    assert!(!b.is_set(0));
    assert!(b.is_set(1));
    assert!(b.is_set(2));
    assert!(b.is_set(3));
    assert_eq!(b.size(), 4);
    assert_eq!(b.get_num_set(), 3);
    assert!(!b.are_all_set());
    assert!(!b.are_all_unset());
    assert!(b.is_any_set());
    assert!(b.is_any_unset());
    assert!(b.are_contiguously_set());
    assert_eq!(b.get_first_set(), 1);
    assert_eq!(b.get_last_set(), 3);
    assert_eq!(b.as_string_left_to_right(), "0111");
    assert_eq!(b.as_string_right_to_left(), "1110");
}

#[test]
fn test_clear_bit() {
    let mut b = CompressedBits::new(4);
    b.set_all();
    b.assign(0, false);
    b.clear(2);

    assert!(!b.is_set(0));
    assert!(b.is_set(1));
    assert!(!b.is_set(2));
    assert!(b.is_set(3));
    assert_eq!(b.size(), 4);
    assert_eq!(b.get_num_set(), 2);
    assert!(!b.are_all_set());
    assert!(!b.are_all_unset());
    assert!(b.is_any_set());
    assert!(b.is_any_unset());
    assert!(!b.are_contiguously_set());
    assert_eq!(b.get_first_set(), 1);
    assert_eq!(b.get_last_set(), 3);
    assert_eq!(b.as_string_left_to_right(), "0101");
    assert_eq!(b.as_string_right_to_left(), "1010");
}

#[test]
fn test_clear_all() {
    let mut b = CompressedBits::new(4);
    b.set_all();
    b.assign(0, false);
    b.clear(2);
    b.clear_all();

    assert_eq!(b.size(), 4);
    assert_eq!(b.get_num_set(), 0);
    assert!(!b.are_all_set());
    assert!(b.are_all_unset());
    assert!(!b.is_any_set());
    assert!(b.is_any_unset());
    assert!(!b.are_contiguously_set());
    assert_eq!(b.get_first_set(), b.size());
    assert_eq!(b.get_last_set(), b.size());
    assert_eq!(b.as_string_left_to_right(), "0000");
    assert_eq!(b.as_string_right_to_left(), "0000");
}

#[test]
fn test_set_range() {
    let mut b = CompressedBits::new(4);
    b.clear_all();
    b.set_range(1, 3);

    assert!(!b.is_set(0));
    assert!(b.is_set(1));
    assert!(b.is_set(2));
    assert!(b.is_set(3));
    assert_eq!(b.size(), 4);
    assert_eq!(b.get_num_set(), 3);
    assert!(!b.are_all_set());
    assert!(!b.are_all_unset());
    assert!(b.is_any_set());
    assert!(b.is_any_unset());
    assert!(b.are_contiguously_set());
    assert_eq!(b.get_first_set(), 1);
    assert_eq!(b.get_last_set(), 3);
    assert_eq!(b.as_string_left_to_right(), "0111");
    assert_eq!(b.as_string_right_to_left(), "1110");
}

#[test]
fn test_set_already_set_bit() {
    let mut b = CompressedBits::new(4);
    b.set_range(1, 3);
    b.set(1);

    assert!(!b.is_set(0));
    assert!(b.is_set(1));
    assert!(b.is_set(2));
    assert!(b.is_set(3));
    assert_eq!(b.size(), 4);
    assert_eq!(b.get_num_set(), 3);
    assert_eq!(b.as_string_left_to_right(), "0111");
}

#[test]
fn test_clear_already_cleared_bit() {
    let mut b = CompressedBits::new(4);
    b.set_range(1, 3);
    b.clear(0);

    assert!(!b.is_set(0));
    assert!(b.is_set(1));
    assert!(b.is_set(2));
    assert!(b.is_set(3));
    assert_eq!(b.size(), 4);
    assert_eq!(b.get_num_set(), 3);
    assert_eq!(b.as_string_left_to_right(), "0111");
}

#[test]
fn test_append() {
    let mut c = CompressedBits::new(0);
    assert_eq!(c.size(), 0);
    assert_eq!(c.get_num_set(), 0);
    assert_eq!(c.as_string_left_to_right(), "");

    c.append(2, false);
    assert_eq!(c.size(), 2);
    assert_eq!(c.get_num_set(), 0);
    assert_eq!(c.as_string_left_to_right(), "00");

    c.append(1, false);
    assert_eq!(c.size(), 3);
    assert_eq!(c.get_num_set(), 0);
    assert_eq!(c.as_string_left_to_right(), "000");

    c.append(2, true);
    assert_eq!(c.size(), 5);
    assert_eq!(c.get_num_set(), 2);
    assert_eq!(c.as_string_left_to_right(), "00011");

    c.append(1, true);
    assert_eq!(c.size(), 6);
    assert_eq!(c.get_num_set(), 3);
    assert_eq!(c.as_string_left_to_right(), "000111");

    c.append(3, false);
    assert_eq!(c.size(), 9);
    assert_eq!(c.get_num_set(), 3);
    assert_eq!(c.as_string_left_to_right(), "000111000");

    c = CompressedBits::new(0);
    assert_eq!(c.size(), 0);
    assert_eq!(c.get_num_set(), 0);
    assert_eq!(c.as_string_left_to_right(), "");

    c.append(3, true);
    assert_eq!(c.size(), 3);
    assert_eq!(c.get_num_set(), 3);
    assert_eq!(c.as_string_left_to_right(), "111");

    let mut d = CompressedBits::new(3);
    d.set_all();
    assert_eq!(c, d);
}

#[test]
fn test_logic_and() {
    let mut a = CompressedBits::new(4);
    a.set_all();
    let mut b = CompressedBits::new(4);

    let c = &a & &b;
    assert!(c.are_all_unset());
    assert_eq!(c.get_num_set(), 0);
    assert_eq!(c.as_string_left_to_right(), "0000");

    let mut c = c;
    c.set(0);
    c.set(1);
    assert_eq!(c.get_num_set(), 2);
    assert_eq!(c.as_string_left_to_right(), "1100");

    c &= &a;
    assert_eq!(c.get_num_set(), 2);
    assert_eq!(c.as_string_left_to_right(), "1100");

    let mut d = a.clone();
    d.clear(0);
    d.clear(2);
    assert_eq!(d.get_num_set(), 2);
    assert_eq!(d.as_string_left_to_right(), "0101");

    c.set(3);
    assert_eq!(c.get_num_set(), 3);
    assert_eq!(c.as_string_left_to_right(), "1101");

    d &= &c;
    assert_eq!(d.get_num_set(), 2);
    assert_eq!(d.as_string_left_to_right(), "0101");

    // OR
    let c = &a | &b;
    assert!(c.are_all_set());
    assert_eq!(c.get_num_set(), 4);
    assert_eq!(c.as_string_left_to_right(), "1111");

    let mut c = c;
    c.clear(0);
    c.clear(1);
    assert_eq!(c.get_num_set(), 2);
    assert_eq!(c.as_string_left_to_right(), "0011");

    c |= &a;
    assert_eq!(c.get_num_set(), 4);
    assert_eq!(c.as_string_left_to_right(), "1111");

    let mut d = a.clone();
    d.clear(0);
    d.clear(2);
    assert_eq!(d.get_num_set(), 2);
    assert_eq!(d.as_string_left_to_right(), "0101");

    c.clear(0);
    c.clear(3);
    assert_eq!(c.get_num_set(), 2);
    assert_eq!(c.as_string_left_to_right(), "0110");

    d |= &c;
    assert_eq!(d.get_num_set(), 3);
    assert_eq!(d.as_string_left_to_right(), "0111");

    // XOR
    let c = &a ^ &b;
    assert!(c.are_all_set());
    assert_eq!(c.get_num_set(), 4);
    assert_eq!(c.as_string_left_to_right(), "1111");

    let mut c = c;
    c.clear(0);
    c.clear(1);
    assert_eq!(c.get_num_set(), 2);
    assert_eq!(c.as_string_left_to_right(), "0011");

    c ^= &a;
    assert_eq!(c.get_num_set(), 2);
    assert_eq!(c.as_string_left_to_right(), "1100");

    // Complement
    a.complement();
    let c = a.clone();
    assert!(c.are_all_unset());
    assert_eq!(c.get_num_set(), 0);
    assert_eq!(c.as_string_left_to_right(), "0000");

    b.complement();
    let c = b.clone();
    assert!(c.are_all_set());
    assert_eq!(c.get_num_set(), 4);
    assert_eq!(c.as_string_left_to_right(), "1111");

    let mut c = c;
    c.clear(0);
    c.clear(2);
    assert_eq!(c.get_num_set(), 2);
    assert_eq!(c.as_string_left_to_right(), "0101");

    c.complement();
    assert_eq!(c.get_num_set(), 2);
    assert_eq!(c.as_string_left_to_right(), "1010");

    // Subtraction
    let mut c = CompressedBits::new(4);
    c.set_all();
    assert_eq!(c.get_num_set(), 4);
    assert_eq!(c.as_string_left_to_right(), "1111");

    let mut d = CompressedBits::new(4);
    d.clear_all();
    assert_eq!(d.get_num_set(), 0);
    assert_eq!(d.as_string_left_to_right(), "0000");

    c -= &d;
    assert_eq!(c.get_num_set(), 4);
    assert_eq!(c.as_string_left_to_right(), "1111");

    d -= &c;
    assert_eq!(d.get_num_set(), 0);
    assert_eq!(d.as_string_left_to_right(), "0000");

    d.set(0);
    assert_eq!(d.get_num_set(), 1);
    assert_eq!(d.as_string_left_to_right(), "1000");

    d -= &c;
    assert_eq!(d.get_num_set(), 0);
    assert_eq!(d.as_string_left_to_right(), "0000");

    d.set(0);
    d.set(2);
    assert_eq!(d.get_num_set(), 2);
    assert_eq!(d.as_string_left_to_right(), "1010");

    d -= &c;
    assert_eq!(d.get_num_set(), 0);
    assert_eq!(d.as_string_left_to_right(), "0000");

    d.set(0);
    d.set(3);
    assert_eq!(d.get_num_set(), 2);
    assert_eq!(d.as_string_left_to_right(), "1001");

    c -= &d;
    assert_eq!(c.get_num_set(), 2);
    assert_eq!(c.as_string_left_to_right(), "0110");

    d.set_all();
    c -= &d;
    assert_eq!(c.get_num_set(), 0);
    assert_eq!(c.as_string_left_to_right(), "0000");
}

#[test]
fn test_logic_randomized() {
    // Randomized cross-check between Bits and CompressedBits for 100 iterations
    // (C++ original ran for 2 seconds; we use a fixed iteration count).

    // Deterministic pseudo-random seed so test is reproducible.
    let mut rng_state: u64 = 0xdeadbeef_cafebabe;
    let mut next_rand = move |max: usize| -> usize {
        // xorshift64
        rng_state ^= rng_state << 13;
        rng_state ^= rng_state >> 7;
        rng_state ^= rng_state << 17;
        (rng_state as usize) % max
    };

    for _ in 0..100 {
        let sz = {
            let v = next_rand(128);
            if v == 0 { 1 } else { v }
        };
        let n_sets = next_rand(sz);

        let mut a = Bits::new(sz);
        let mut b = Bits::new(sz);
        let mut ca = CompressedBits::new(sz);
        let mut cb = CompressedBits::new(sz);

        assert_eq!(a.as_string_left_to_right(), ca.as_string_left_to_right());
        assert_eq!(b.as_string_left_to_right(), cb.as_string_left_to_right());

        for _ in 0..n_sets {
            let idx_a = next_rand(sz);
            a.set(idx_a);
            ca.set(idx_a);
            assert_eq!(a.as_string_left_to_right(), ca.as_string_left_to_right());

            let idx_b = next_rand(sz);
            b.set(idx_b);
            cb.set(idx_b);
            assert_eq!(b.as_string_left_to_right(), cb.as_string_left_to_right());

            // complement (double to restore)
            a.complement();
            ca.complement();
            assert_eq!(a.as_string_left_to_right(), ca.as_string_left_to_right());
            a.complement();
            ca.complement();
            assert_eq!(a.as_string_left_to_right(), ca.as_string_left_to_right());

            b.complement();
            cb.complement();
            assert_eq!(b.as_string_left_to_right(), cb.as_string_left_to_right());
            b.complement();
            cb.complement();
            assert_eq!(b.as_string_left_to_right(), cb.as_string_left_to_right());

            assert_eq!(
                (&a | &b).as_string_left_to_right(),
                (&ca | &cb).as_string_left_to_right()
            );
            assert_eq!(
                (&a & &b).as_string_left_to_right(),
                (&ca & &cb).as_string_left_to_right()
            );
            assert_eq!(
                (&a ^ &b).as_string_left_to_right(),
                (&ca ^ &cb).as_string_left_to_right()
            );

            // subtraction: a & !b  vs  ca - cb
            let mut tmp_b = b.clone();
            tmp_b.complement();
            let sub_bits = &a & &tmp_b;
            let sub_cbits = &ca - &cb;
            assert_eq!(
                sub_bits.as_string_left_to_right(),
                sub_cbits.as_string_left_to_right()
            );

            // in-place ops
            a |= &b;
            ca |= &cb;
            assert_eq!(a.as_string_left_to_right(), ca.as_string_left_to_right());

            a &= &b;
            ca &= &cb;
            assert_eq!(a.as_string_left_to_right(), ca.as_string_left_to_right());

            a ^= &b;
            ca ^= &cb;
            assert_eq!(a.as_string_left_to_right(), ca.as_string_left_to_right());

            a -= &b;
            ca -= &cb;
            assert_eq!(a.as_string_left_to_right(), ca.as_string_left_to_right());

            assert_eq!(a.contains(&b), ca.contains(&cb));
            assert_eq!(
                a.has_non_empty_difference(&b),
                ca.has_non_empty_difference(&cb)
            );
            assert_eq!(
                a.has_non_empty_intersection(&b),
                ca.has_non_empty_intersection(&cb)
            );

            assert_eq!(a.get_first_set(), ca.get_first_set());
            assert_eq!(b.get_first_set(), cb.get_first_set());
            assert_eq!(a.get_last_set(), ca.get_last_set());
            assert_eq!(b.get_last_set(), cb.get_last_set());
            assert_eq!(a.get_num_set(), ca.get_num_set());
            assert_eq!(b.get_num_set(), cb.get_num_set());

            assert_eq!(a.as_string_left_to_right(), ca.as_string_left_to_right());
            assert_eq!(b.as_string_left_to_right(), cb.as_string_left_to_right());
        }
    }
}

#[test]
fn test_contains_and_overlaps() {
    let mut a = CompressedBits::new(4);
    a.set_range(1, 3);
    assert_eq!(a.as_string_left_to_right(), "0111");

    // Contains (HasNonEmptyDifference)
    let mut b = CompressedBits::new(4);
    b.set(0);
    assert_eq!(b.as_string_left_to_right(), "1000");
    assert!(b.has_non_empty_difference(&a));

    b.set(1);
    assert_eq!(b.as_string_left_to_right(), "1100");
    assert!(b.has_non_empty_difference(&a));

    b.set(2);
    assert_eq!(b.as_string_left_to_right(), "1110");
    assert!(b.has_non_empty_difference(&a));

    b.clear(0);
    assert_eq!(b.as_string_left_to_right(), "0110");
    assert!(!b.has_non_empty_difference(&a));

    b.clear(1);
    assert_eq!(b.as_string_left_to_right(), "0010");
    assert!(!b.has_non_empty_difference(&a));

    b.clear(2);
    assert_eq!(b.as_string_left_to_right(), "0000");
    assert!(!b.has_non_empty_difference(&a));

    a.clear(3);
    b.set(3);
    assert_eq!(a.as_string_left_to_right(), "0110");
    assert_eq!(b.as_string_left_to_right(), "0001");
    assert!(b.has_non_empty_difference(&a));

    // Overlaps (HasNonEmptyIntersection)
    assert!(!b.has_non_empty_intersection(&a));

    a.set(3);
    assert_eq!(a.as_string_left_to_right(), "0111");
    assert!(b.has_non_empty_intersection(&a));

    b.clear(3);
    assert_eq!(b.as_string_left_to_right(), "0000");
    assert!(!b.has_non_empty_intersection(&a));

    b.set(2);
    assert_eq!(b.as_string_left_to_right(), "0010");
    assert!(b.has_non_empty_intersection(&a));

    b.set(1);
    assert_eq!(b.as_string_left_to_right(), "0110");
    assert!(b.has_non_empty_intersection(&a));

    b.set(3);
    assert_eq!(b.as_string_left_to_right(), "0111");
    assert!(b.has_non_empty_intersection(&a));

    let mut c = CompressedBits::new(4);
    c.set(0);
    assert_eq!(c.as_string_left_to_right(), "1000");
    assert!(!b.has_non_empty_intersection(&c));
}

#[test]
fn test_platform_counts() {
    let mut a = CompressedBits::new(10);
    assert_eq!(a.get_num_set(), 0);
    assert_eq!(a.as_string_left_to_right(), "0000000000");
    assert_eq!(a.get_num_platforms(), 1);
    assert_eq!(a.get_num_set_platforms(), 0);
    assert_eq!(a.get_num_unset_platforms(), 1);

    a.set(0);
    assert_eq!(a.get_num_set(), 1);
    assert_eq!(a.as_string_left_to_right(), "1000000000");
    assert_eq!(a.get_num_platforms(), 2);
    assert_eq!(a.get_num_set_platforms(), 1);
    assert_eq!(a.get_num_unset_platforms(), 1);

    a.set(2);
    assert_eq!(a.get_num_set(), 2);
    assert_eq!(a.as_string_left_to_right(), "1010000000");
    assert_eq!(a.get_num_platforms(), 4);
    assert_eq!(a.get_num_set_platforms(), 2);
    assert_eq!(a.get_num_unset_platforms(), 2);

    a.set(4);
    assert_eq!(a.get_num_set(), 3);
    assert_eq!(a.as_string_left_to_right(), "1010100000");
    assert_eq!(a.get_num_platforms(), 6);
    assert_eq!(a.get_num_set_platforms(), 3);
    assert_eq!(a.get_num_unset_platforms(), 3);

    a.set(6);
    assert_eq!(a.get_num_set(), 4);
    assert_eq!(a.as_string_left_to_right(), "1010101000");
    assert_eq!(a.get_num_platforms(), 8);
    assert_eq!(a.get_num_set_platforms(), 4);
    assert_eq!(a.get_num_unset_platforms(), 4);

    a.set(8);
    assert_eq!(a.get_num_set(), 5);
    assert_eq!(a.as_string_left_to_right(), "1010101010");
    assert_eq!(a.get_num_platforms(), 10);
    assert_eq!(a.get_num_set_platforms(), 5);
    assert_eq!(a.get_num_unset_platforms(), 5);

    // Fill alternate bits to consolidate platforms
    a.set(1);
    assert_eq!(a.get_num_set(), 6);
    assert_eq!(a.as_string_left_to_right(), "1110101010");
    assert_eq!(a.get_num_platforms(), 8);
    assert_eq!(a.get_num_set_platforms(), 4);
    assert_eq!(a.get_num_unset_platforms(), 4);

    a.set(3);
    assert_eq!(a.get_num_set(), 7);
    assert_eq!(a.as_string_left_to_right(), "1111101010");
    assert_eq!(a.get_num_platforms(), 6);
    assert_eq!(a.get_num_set_platforms(), 3);
    assert_eq!(a.get_num_unset_platforms(), 3);

    a.set(5);
    assert_eq!(a.get_num_set(), 8);
    assert_eq!(a.as_string_left_to_right(), "1111111010");
    assert_eq!(a.get_num_platforms(), 4);
    assert_eq!(a.get_num_set_platforms(), 2);
    assert_eq!(a.get_num_unset_platforms(), 2);

    a.set(7);
    assert_eq!(a.get_num_set(), 9);
    assert_eq!(a.as_string_left_to_right(), "1111111110");
    assert_eq!(a.get_num_platforms(), 2);
    assert_eq!(a.get_num_set_platforms(), 1);
    assert_eq!(a.get_num_unset_platforms(), 1);

    a.set(9);
    assert_eq!(a.get_num_set(), 10);
    assert_eq!(a.as_string_left_to_right(), "1111111111");
    assert_eq!(a.get_num_platforms(), 1);
    assert_eq!(a.get_num_set_platforms(), 1);
    assert_eq!(a.get_num_unset_platforms(), 0);
}

#[test]
fn test_iterators_all() {
    let mut c = CompressedBits::new(8);
    c.set(1);
    c.set(2);
    c.set(3);
    c.set(6);
    c.set(7);

    assert_eq!(c.get_num_set(), 5);
    assert_eq!(c.as_string_left_to_right(), "01110011");
    assert_eq!(c.get_first_set(), 1);
    assert_eq!(c.get_last_set(), 7);
    assert!(c.is_any_set());
    assert!(c.is_any_unset());
    assert!(!c.are_all_set());
    assert!(!c.are_all_unset());
    assert!(!c.are_contiguously_set());

    // Individual values
    assert!(!c.is_set(0));
    assert!(c.is_set(1));
    assert!(c.is_set(2));
    assert!(c.is_set(3));
    assert!(!c.is_set(4));
    assert!(!c.is_set(5));
    assert!(c.is_set(6));
    assert!(c.is_set(7));

    // All iterator: count = 8, sum of indices = 0+1+2+3+4+5+6+7 = 28,
    // sum of IsSet values = 5
    let all_items: Vec<(usize, bool)> = c.iter_all().collect();
    assert_eq!(all_items.len(), 8);
    let all_sum: usize = all_items.iter().map(|(i, _)| i).sum();
    assert_eq!(all_sum, 28);
    let is_set_sum: usize = all_items.iter().map(|(_, set)| *set as usize).sum();
    assert_eq!(is_set_sum, 5);

    // All Set: count = 5, sum = 1+2+3+6+7 = 19, all IsSet = 5
    let set_items: Vec<usize> = c.iter_set().collect();
    assert_eq!(set_items.len(), 5);
    let set_sum: usize = set_items.iter().sum();
    assert_eq!(set_sum, 19);

    // All Unset: count = 3, sum = 0+4+5 = 9, all IsSet = 0
    let unset_items: Vec<usize> = c.iter_unset().collect();
    assert_eq!(unset_items.len(), 3);
    let unset_sum: usize = unset_items.iter().sum();
    assert_eq!(unset_sum, 9);

    // Platforms: count = 4 (0unset, 1-3set, 4-5unset, 6-7set)
    // start indices: 0, 1, 4, 6 → sum = 11
    // IsSet values: 0, 1, 0, 1 → sum = 2
    // platform sizes: 1, 3, 2, 2 → sum = 8
    // iter_platforms() yields (start: usize, length: usize, is_set: bool)
    let platforms: Vec<(usize, usize, bool)> = c.iter_platforms().collect();
    assert_eq!(platforms.len(), 4);
    let platform_start_sum: usize = platforms.iter().map(|&(start, _, _)| start).sum();
    assert_eq!(platform_start_sum, 11);
    let platform_set_sum: usize = platforms
        .iter()
        .map(|&(_, _, is_set)| is_set as usize)
        .sum();
    assert_eq!(platform_set_sum, 2);
    let platform_size_sum: usize = platforms.iter().map(|&(_, length, _)| length).sum();
    assert_eq!(platform_size_sum, 8);
}

#[test]
fn test_iterator_empty_mask() {
    let d = CompressedBits::new(8);
    // Verify no bits are set
    assert_eq!(d.get_num_set(), 0);

    let e = CompressedBits::new(0);
    let set_items: Vec<usize> = e.iter_set().collect();
    assert_eq!(set_items.len(), 0);
}

#[test]
fn test_iterator_all_ones_mask() {
    let mut d = CompressedBits::new(8);
    d.set_all();
    let count: usize = d.iter_set().count();
    assert_eq!(count, 8);

    let mut e = CompressedBits::new(1);
    e.set_all();
    let count: usize = e.iter_set().count();
    assert_eq!(count, 1);
}

#[test]
fn test_find_next_prev_set_unset() {
    let mut c = CompressedBits::new(8);
    c.set(1);
    c.set(2);
    c.set(3);
    c.set(6);
    c.set(7);

    // FindNextSet: forward accumulate = 1+2+3+6+7 = 19
    let mut accum = 0usize;
    let mut i = c.get_first_set();
    while i < c.size() {
        accum += i;
        i = c.find_next_set(i + 1);
    }
    assert_eq!(accum, 19);

    // FindPrevSet: backward accumulate = 7+6+3+2+1 = 19
    accum = 0;
    let mut i_opt = if c.get_last_set() < c.size() {
        Some(c.get_last_set())
    } else {
        None
    };
    while let Some(i) = i_opt {
        accum += i;
        if i == 0 {
            break;
        }
        i_opt = c.find_prev_set(i - 1);
    }
    assert_eq!(accum, 19);

    // FindNextUnset: forward accumulate = 0+4+5 = 9
    accum = 0;
    i = 0;
    while i < c.size() {
        accum += i;
        i = c.find_next_unset(i + 1);
    }
    assert_eq!(accum, 9);
}

#[test]
fn test_find_nth_set() {
    let mut c = CompressedBits::new(8);
    c.set(1);
    c.set(2);
    c.set(3);
    c.set(6);
    c.set(7);
    // 01110011

    assert_eq!(c.find_nth_set(0), 1);
    assert_eq!(c.find_nth_set(1), 2);
    assert_eq!(c.find_nth_set(2), 3);
    assert_eq!(c.find_nth_set(3), 6);
    assert_eq!(c.find_nth_set(4), 7);
    assert_eq!(c.find_nth_set(5), c.size());
    assert_eq!(c.find_nth_set(6), c.size());
    assert_eq!(c.find_nth_set(100), c.size());

    // 10001100 (complement)
    let mut ic = c.clone();
    ic.complement();
    assert_eq!(ic.find_nth_set(0), 0);
    assert_eq!(ic.find_nth_set(1), 4);
    assert_eq!(ic.find_nth_set(2), 5);
    assert_eq!(ic.find_nth_set(3), ic.size());
    assert_eq!(ic.find_nth_set(4), ic.size());
    assert_eq!(ic.find_nth_set(100), ic.size());

    // 1111 (all set)
    let mut ac = CompressedBits::new(4);
    ac.set_all();
    assert_eq!(ac.find_nth_set(0), 0);
    assert_eq!(ac.find_nth_set(1), 1);
    assert_eq!(ac.find_nth_set(2), 2);
    assert_eq!(ac.find_nth_set(3), 3);
    assert_eq!(ac.find_nth_set(4), ac.size());
    assert_eq!(ac.find_nth_set(100), ac.size());

    // 0000 (all unset)
    let nc = CompressedBits::new(4);
    assert_eq!(nc.find_nth_set(0), nc.size());
    assert_eq!(nc.find_nth_set(1), nc.size());
    assert_eq!(nc.find_nth_set(2), nc.size());
    assert_eq!(nc.find_nth_set(3), nc.size());
    assert_eq!(nc.find_nth_set(4), nc.size());
    assert_eq!(nc.find_nth_set(100), nc.size());
}

#[test]
fn test_compress_decompress() {
    let mut c = Bits::new(10);
    c.set(1);
    c.set(2);
    c.set(6);
    c.set(7);
    c.set(8);
    assert_eq!(c.as_string_left_to_right(), "0110001110");

    let cc = CompressedBits::from(&c as &Bits);
    assert_eq!(cc.as_string_left_to_right(), "0110001110");
    assert_eq!(cc.get_num_set(), 5);

    c.complement();
    assert_eq!(c.as_string_left_to_right(), "1001110001");

    let cc = CompressedBits::from(&c as &Bits);
    assert_eq!(cc.as_string_left_to_right(), "1001110001");

    let d = cc.to_bits();
    assert_eq!(d.as_string_left_to_right(), "1001110001");

    // 1x1 and 1x0 masks
    let mut e = Bits::new(1);
    assert_eq!(e.as_string_left_to_right(), "0");

    let cc = CompressedBits::from(&e as &Bits);
    assert_eq!(cc.size(), 1);
    assert_eq!(cc.get_num_set(), 0);
    assert!(!cc.is_set(0));
    assert_eq!(cc.as_string_left_to_right(), "0");

    e.set_all();
    assert_eq!(e.as_string_left_to_right(), "1");

    let cc = CompressedBits::from(&e as &Bits);
    assert_eq!(cc.size(), 1);
    assert_eq!(cc.get_num_set(), 1);
    assert!(cc.is_set(0));
    assert_eq!(cc.as_string_left_to_right(), "1");
}

#[test]
fn test_shift_right() {
    let mut c = CompressedBits::new(8);
    c.set(2);
    c.set(3);
    c.set(4);
    c.set(6);
    assert_eq!(c.as_string_left_to_right(), "00111010");
    assert_eq!(c.get_num_set(), 4);

    c.shift_right(0);
    assert_eq!(c.as_string_left_to_right(), "00111010");
    assert_eq!(c.get_num_set(), 4);

    c.shift_right(1);
    assert_eq!(c.as_string_left_to_right(), "00011101");
    assert_eq!(c.get_num_set(), 4);

    c.shift_right(1);
    assert_eq!(c.as_string_left_to_right(), "00001110");
    assert_eq!(c.get_num_set(), 3);

    c.shift_right(2);
    assert_eq!(c.as_string_left_to_right(), "00000011");
    assert_eq!(c.get_num_set(), 2);

    c.shift_right(5);
    assert_eq!(c.as_string_left_to_right(), "00000000");
    assert_eq!(c.get_num_set(), 0);

    c.set(0);
    c.set(1);
    c.set(2);
    c.set(3);
    c.set(6);
    c.set(7);
    assert_eq!(c.as_string_left_to_right(), "11110011");
    assert_eq!(c.get_num_set(), 6);

    c.shift_right(3);
    assert_eq!(c.as_string_left_to_right(), "00011110");
    assert_eq!(c.get_num_set(), 4);

    c.shift_right(3);
    assert_eq!(c.as_string_left_to_right(), "00000011");
    assert_eq!(c.get_num_set(), 2);

    c.shift_right(2);
    assert_eq!(c.as_string_left_to_right(), "00000000");
    assert_eq!(c.get_num_set(), 0);

    c.complement();
    assert_eq!(c.as_string_left_to_right(), "11111111");
    assert_eq!(c.get_num_set(), 8);

    c.shift_right(4);
    assert_eq!(c.as_string_left_to_right(), "00001111");
    assert_eq!(c.get_num_set(), 4);

    c.shift_right(100);
    assert_eq!(c.as_string_left_to_right(), "00000000");
    assert_eq!(c.get_num_set(), 0);

    c.shift_right(100);
    assert_eq!(c.as_string_left_to_right(), "00000000");
    assert_eq!(c.get_num_set(), 0);
}

#[test]
fn test_shift_left() {
    let mut c = CompressedBits::new(8);
    c.set(2);
    c.set(3);
    c.set(4);
    c.set(6);
    assert_eq!(c.as_string_left_to_right(), "00111010");
    assert_eq!(c.get_num_set(), 4);

    c.shift_left(0);
    assert_eq!(c.as_string_left_to_right(), "00111010");
    assert_eq!(c.get_num_set(), 4);

    c.shift_left(1);
    assert_eq!(c.as_string_left_to_right(), "01110100");
    assert_eq!(c.get_num_set(), 4);

    c.shift_left(1);
    assert_eq!(c.as_string_left_to_right(), "11101000");
    assert_eq!(c.get_num_set(), 4);

    c.shift_left(2);
    assert_eq!(c.as_string_left_to_right(), "10100000");
    assert_eq!(c.get_num_set(), 2);

    c.shift_left(5);
    assert_eq!(c.as_string_left_to_right(), "00000000");
    assert_eq!(c.get_num_set(), 0);

    c.set(0);
    c.set(1);
    c.set(2);
    c.set(3);
    c.set(6);
    c.set(7);
    assert_eq!(c.as_string_left_to_right(), "11110011");
    assert_eq!(c.get_num_set(), 6);

    c.shift_left(3);
    assert_eq!(c.as_string_left_to_right(), "10011000");
    assert_eq!(c.get_num_set(), 3);

    c.shift_left(3);
    assert_eq!(c.as_string_left_to_right(), "11000000");
    assert_eq!(c.get_num_set(), 2);

    c.shift_left(2);
    assert_eq!(c.as_string_left_to_right(), "00000000");
    assert_eq!(c.get_num_set(), 0);

    c.complement();
    assert_eq!(c.as_string_left_to_right(), "11111111");
    assert_eq!(c.get_num_set(), 8);

    c.shift_left(4);
    assert_eq!(c.as_string_left_to_right(), "11110000");
    assert_eq!(c.get_num_set(), 4);

    c.shift_left(100);
    assert_eq!(c.as_string_left_to_right(), "00000000");
    assert_eq!(c.get_num_set(), 0);

    c.shift_left(100);
    assert_eq!(c.as_string_left_to_right(), "00000000");
    assert_eq!(c.get_num_set(), 0);
}

#[test]
fn test_resize_keep_contents() {
    let mut b = CompressedBits::new(6);
    b.set(0);
    b.set(1);
    b.set(4);
    assert_eq!(b.as_string_left_to_right(), "110010");

    b.resize_keep_contents(6);
    assert_eq!(b.as_string_left_to_right(), "110010");

    b.resize_keep_contents(10);
    assert_eq!(b.as_string_left_to_right(), "1100100000");

    b.resize_keep_contents(6);
    assert_eq!(b.as_string_left_to_right(), "110010");

    b.resize_keep_contents(2);
    assert_eq!(b.as_string_left_to_right(), "11");

    b.resize_keep_contents(1);
    assert_eq!(b.as_string_left_to_right(), "1");

    b.resize_keep_contents(0);
    assert_eq!(b.size(), 0);
    assert_eq!(b.get_num_set(), 0);
}

#[test]
fn test_bits_api_compatibility_empty() {
    let a = Bits::new(0);
    let b = CompressedBits::new(0);
    verify_equality_with_conversion(&a, &b);
}

#[test]
fn test_bits_api_compatibility_empty_all_set() {
    let mut a = Bits::new(0);
    let mut b = CompressedBits::new(0);
    a.set_all();
    b.set_all();
    verify_equality_with_conversion(&a, &b);
}

#[test]
fn test_bits_api_compatibility_size1() {
    let a = Bits::new(1);
    let b = CompressedBits::new(1);
    verify_equality_with_conversion(&a, &b);
}

#[test]
fn test_bits_api_compatibility_size1_all_set() {
    let mut a = Bits::new(1);
    let mut b = CompressedBits::new(1);
    a.set_all();
    b.set_all();
    verify_equality_with_conversion(&a, &b);
}

#[test]
fn test_bits_api_compatibility_size4() {
    let a = Bits::new(4);
    let b = CompressedBits::new(4);
    verify_equality_with_conversion(&a, &b);
}

#[test]
fn test_bits_api_compatibility_size4_all_set() {
    let mut a = Bits::new(4);
    let mut b = CompressedBits::new(4);
    a.set_all();
    b.set_all();
    verify_equality_with_conversion(&a, &b);
}

#[test]
fn test_bits_api_compatibility_clear_and_complement() {
    let mut a = Bits::new(4);
    let mut b = CompressedBits::new(4);
    a.set_all();
    b.set_all();

    a.clear(0);
    a.clear(3);
    b.clear(0);
    b.clear(3);
    a.set_all();
    b.set_all();
    verify_equality_with_conversion(&a, &b);

    a.complement();
    b.complement();
    verify_equality_with_conversion(&a, &b);
}

#[test]
fn test_set_range_consistency_bug() {
    // Regression: CompressedBits left in inconsistent state when platforms
    // array would contain zeroes while _num was nonzero.
    let mut a = CompressedBits::new(4);
    a.set_range(0, 3);

    let mut b = CompressedBits::new(4);
    b.set_all();

    assert_eq!(a, b);

    a.clear_all();
    a.set_range(2, 3);
    b.clear(0);
    b.clear(1);
    assert_eq!(a, b);
}

#[test]
fn test_from_string_rle_format() {
    let c = CompressedBits::from_string("0x5-1x5-0x5");
    assert_eq!(c.as_string_left_to_right(), "000001111100000");
    assert_eq!(c.as_rle_string(), "0x5-1x5-0x5");
}

#[test]
fn test_from_string_rle_with_whitespace() {
    let c = CompressedBits::from_string("  0x5 - 1x5 - 0 x 5  ");
    assert_eq!(c.as_string_left_to_right(), "000001111100000");
    assert_eq!(c.as_rle_string(), "0x5-1x5-0x5");
}

#[test]
fn test_from_string_binary_format() {
    let c = CompressedBits::from_string("000001111100000");
    assert_eq!(c.as_string_left_to_right(), "000001111100000");
    assert_eq!(c.as_rle_string(), "0x5-1x5-0x5");
}

#[test]
fn test_from_string_binary_with_spaces() {
    let c = CompressedBits::from_string("00000 11111 000 00");
    assert_eq!(c.as_string_left_to_right(), "000001111100000");
    assert_eq!(c.as_rle_string(), "0x5-1x5-0x5");
}

#[test]
fn test_from_string_all_zeros_rle() {
    let c = CompressedBits::from_string("0x15");
    assert_eq!(c.as_string_left_to_right(), "000000000000000");
    assert_eq!(c.as_rle_string(), "0x15");
}

#[test]
fn test_from_string_all_ones_rle() {
    let c = CompressedBits::from_string("1x15");
    assert_eq!(c.as_string_left_to_right(), "111111111111111");
    assert_eq!(c.as_rle_string(), "1x15");
}

#[test]
fn test_from_string_invalid_digit() {
    let c = CompressedBits::from_string("3x15");
    assert_eq!(c.size(), 0);
}

#[test]
fn test_from_string_invalid_zero_count() {
    let c = CompressedBits::from_string("1x0");
    assert_eq!(c.size(), 0);
}

#[test]
fn test_from_string_invalid_double_x() {
    let c = CompressedBits::from_string("0x5x1");
    assert_eq!(c.size(), 0);
}

#[test]
fn test_from_string_invalid_truncated_rle() {
    let c = CompressedBits::from_string("0x5-1");
    assert_eq!(c.size(), 0);
}

#[test]
fn test_from_string_invalid_separator_position() {
    let c = CompressedBits::from_string("0-5x1");
    assert_eq!(c.size(), 0);
}

#[test]
fn test_from_string_invalid_garbage() {
    let c = CompressedBits::from_string("foo bar");
    assert_eq!(c.size(), 0);
}

#[test]
fn test_from_string_invalid_trailing_garbage() {
    let c = CompressedBits::from_string("1x15 foo");
    assert_eq!(c.size(), 0);
}

#[test]
fn test_from_string_invalid_non_binary_digit() {
    let c = CompressedBits::from_string("000001111122222");
    assert_eq!(c.size(), 0);
}
