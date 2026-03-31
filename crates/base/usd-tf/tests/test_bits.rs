// Port of pxr/base/tf/testenv/bits.cpp

use usd_tf::bits::Bits;

fn test_swap(a: &Bits, b: &mut Bits) {
    // Make copies so we can test swapping in both directions.
    let mut a1 = a.clone();
    let mut a2 = a.clone();
    let mut b1 = b.clone();
    let mut b2 = b.clone();

    b1.swap(&mut a1);
    assert_eq!(a1, *b);
    assert_eq!(b1, *a);

    a2.swap(&mut b2);
    assert_eq!(a2, *b);
    assert_eq!(b2, *a);

    // Swap back
    b1.swap(&mut a1);
    assert_eq!(a1, *a);
    assert_eq!(b1, *b);

    a2.swap(&mut b2);
    assert_eq!(a2, *a);
    assert_eq!(b2, *b);
}

#[test]
fn test_basic_construction_and_queries() {
    let b = Bits::new(4);

    assert_eq!(b.get_size(), 4);
    assert_eq!(b.get_num_set(), 0);
    assert!(!b.are_all_set());
    assert!(b.are_all_unset());
    assert!(!b.are_contiguously_set());
}

#[test]
fn test_set_single_bit() {
    let mut b = Bits::new(4);
    b.set(0);

    assert_eq!(b.get_size(), 4);
    assert_eq!(b.get_num_set(), 1);
    assert!(!b.are_all_set());
    assert!(!b.are_all_unset());
    assert!(b.are_contiguously_set());

    assert_eq!(b.as_string_left_to_right(), "1000");
    assert_eq!(b.as_string_right_to_left(), "0001");
}

#[test]
fn test_resize_keep_content_grow() {
    let mut b = Bits::new(4);
    b.set(0);

    b.resize_keep_content(8);
    assert_eq!(b.get_size(), 8);
    assert_eq!(b.get_num_set(), 1);
    assert!(!b.are_all_set());
    assert!(!b.are_all_unset());
    assert!(b.are_contiguously_set());

    assert_eq!(b.as_string_left_to_right(), "10000000");
    assert_eq!(b.as_string_right_to_left(), "00000001");
}

#[test]
fn test_resize_keep_content_shrink() {
    let mut b = Bits::new(8);
    b.set(0);

    b.resize_keep_content(2);
    assert_eq!(b.get_size(), 2);
    assert_eq!(b.get_num_set(), 1);
    assert!(!b.are_all_set());
    assert!(!b.are_all_unset());
    assert!(b.are_contiguously_set());

    assert_eq!(b.as_string_left_to_right(), "10");
    assert_eq!(b.as_string_right_to_left(), "01");
}

#[test]
fn test_assign_api() {
    let mut a = Bits::new(4);
    a.clear_all();
    a.set(1);
    assert_eq!(a.as_string_left_to_right(), "0100");

    a.assign(2, true);
    assert_eq!(a.as_string_left_to_right(), "0110");
    assert_eq!(a.get_num_set(), 2);
    assert_eq!(a.get_first_set(), 1);
    assert_eq!(a.get_last_set(), 2);

    a.assign(2, false);
    assert_eq!(a.as_string_left_to_right(), "0100");
    assert_eq!(a.get_num_set(), 1);
    assert_eq!(a.get_first_set(), 1);
    assert_eq!(a.get_last_set(), 1);

    a.assign(3, false);
    assert_eq!(a.as_string_left_to_right(), "0100");
    assert_eq!(a.get_num_set(), 1);
    assert_eq!(a.get_first_set(), 1);
    assert_eq!(a.get_last_set(), 1);

    let mut t = Bits::new(0);
    t.resize(12);
    t.clear_all();
    t.assign(1, true);
    t.assign(2, true);
    assert_eq!(t.as_string_left_to_right(), "011000000000");
    t.assign(4, false);
    t.assign(5, true);
    assert_eq!(t.as_string_left_to_right(), "011001000000");
    assert_eq!(t.get_num_set(), 3);
    assert_eq!(t.get_first_set(), 1);
    assert_eq!(t.get_last_set(), 5);
}

#[test]
fn test_resize_zero_then_get_first_set() {
    // Regression: GetFirstSet() after ResizeKeepContent() on zero-sized array.
    let mut b = Bits::new(0);
    assert_eq!(b.get_first_set(), 0);
    b.resize_keep_content(4);
    b.assign(3, true);
    assert_eq!(b.get_first_set(), 3);
}

#[test]
fn test_iterators() {
    let mut b = Bits::new(4);
    b.resize_keep_content(0);
    b.resize_keep_content(4);
    b.assign(3, true);
    b.assign(1, true);
    assert_eq!(b.as_string_left_to_right(), "0101");

    // All iterator: sum of all indices (0+1+2+3 = 6)
    let all_sum: usize = b.iter().sum();
    assert_eq!(all_sum, 6);

    // Set iterator: sum of set indices (1+3 = 4)
    let set_sum: usize = b.iter_set().sum();
    assert_eq!(set_sum, 4);

    // Unset iterator: sum of unset indices (0+2 = 2)
    let unset_sum: usize = b.iter_unset().sum();
    assert_eq!(unset_sum, 2);
}

#[test]
fn test_swap_both_small() {
    // Both arrays small enough for inline storage.
    let mut a = Bits::new(4);
    let mut b = Bits::new(2);
    a.set(0);
    b.set(1);

    test_swap(&a, &mut b);
}

#[test]
fn test_swap_both_large() {
    // Both arrays large enough for heap-allocated storage.
    let mut a = Bits::new(2048);
    let mut b = Bits::new(1024);
    a.set(0);
    b.set(512);

    test_swap(&a, &mut b);
}

#[test]
fn test_swap_mixed_storage() {
    // 'a' uses inline storage, 'b' uses heap storage.
    let mut a = Bits::new(4);
    let mut b = Bits::new(1024);
    a.set(0);
    b.set(512);

    test_swap(&a, &mut b);
}
