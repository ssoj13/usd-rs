/// Integration tests for small_vector.rs - port of testenv/smallVector.cpp
///
/// Tests constructors, inline/heap transitions, push/pop, reserve,
/// iteration, insertion, erasure, and resize.
///
/// Note: SmallVec does not expose drain/dedup directly; those are implemented
/// via remove-range loops and retain+sort, which are equivalent semantically.
use usd_tf::small_vector::SmallVec;

// ---------------------------------------------------------------------------
// Constructors
// ---------------------------------------------------------------------------

#[test]
fn default_constructor() {
    let v: SmallVec<i32, 1> = SmallVec::new();
    assert_eq!(v.len(), 0);
    assert_eq!(v.capacity(), 1);
    assert!(v.is_empty());
}

#[test]
fn fill_constructor_within_inline() {
    let v: SmallVec<i32, 2> = SmallVec::from_iter_items(std::iter::repeat(14).take(2));
    assert_eq!(v.len(), 2);
    assert_eq!(v.capacity(), 2);
    assert_eq!(v[0], 14);
    assert_eq!(v[1], 14);
}

#[test]
fn fill_constructor_partial_inline() {
    // Inline capacity 2, only 1 element — capacity should still be 2.
    let v: SmallVec<i32, 2> = SmallVec::from_iter_items(std::iter::repeat(15).take(1));
    assert_eq!(v.len(), 1);
    assert_eq!(v.capacity(), 2);
    assert_eq!(v[0], 15);
}

#[test]
fn fill_constructor_spills_to_heap() {
    // N=1 but 10 elements => must go to heap.
    let v: SmallVec<i32, 1> = SmallVec::from_iter_items(std::iter::repeat(15).take(10));
    assert_eq!(v.len(), 10);
    assert!(v.capacity() >= 10);
    for &x in v.iter() {
        assert_eq!(x, 15);
    }
    assert!(v.is_heap());
}

#[test]
fn copy_constructor_inline() {
    let v1: SmallVec<i32, 2> = SmallVec::from_iter_items([14, 14]);
    let v2 = v1.clone();
    assert_eq!(v2.len(), 2);
    assert_eq!(v2[0], 14);
    assert_eq!(v2[1], 14);
}

#[test]
fn copy_constructor_heap() {
    let v1: SmallVec<i32, 1> = SmallVec::from_iter_items(std::iter::repeat(15).take(10));
    let v2 = v1.clone();
    assert_eq!(v2.len(), 10);
    for &x in v2.iter() {
        assert_eq!(x, 15);
    }
}

#[test]
fn move_constructor_inline() {
    let v1: SmallVec<i32, 2> = SmallVec::from_iter_items([14, 14]);
    let v2 = v1; // move
    assert_eq!(v2.len(), 2);
    assert_eq!(v2[0], 14);
    assert_eq!(v2[1], 14);
}

#[test]
fn move_constructor_heap() {
    let v1: SmallVec<i32, 1> = SmallVec::from_iter_items(std::iter::repeat(15).take(10));
    let v2 = v1; // move
    assert_eq!(v2.len(), 10);
    for &x in v2.iter() {
        assert_eq!(x, 15);
    }
}

#[test]
fn iter_range_constructor_full_heap() {
    let source: Vec<i32> = vec![42; 100];
    let v: SmallVec<i32, 1> = SmallVec::from_iter_items(source.iter().copied());
    assert_eq!(v.len(), 100);
    assert!(v.capacity() >= 100);
    for (i, &x) in v.iter().enumerate() {
        assert_eq!(x, source[i]);
    }
}

#[test]
fn iter_range_constructor_fits_inline() {
    // 10 elements, N=10: should fit inline.
    let source: Vec<i32> = (0..10).collect();
    let v: SmallVec<i32, 10> = SmallVec::from_iter_items(source.iter().copied());
    assert_eq!(v.len(), 10);
    assert_eq!(v.capacity(), 10);
    assert!(!v.is_heap());
}

#[test]
fn iter_range_constructor_inline_larger_than_count() {
    // 10 elements, N=15: inline cap 15 but only 10 filled.
    let source: Vec<i32> = (0..10).collect();
    let v: SmallVec<i32, 15> = SmallVec::from_iter_items(source.iter().copied());
    assert_eq!(v.len(), 10);
    assert_eq!(v.capacity(), 15);
    assert!(!v.is_heap());
}

// ---------------------------------------------------------------------------
// Initializer list construction
// ---------------------------------------------------------------------------

#[test]
fn initializer_list_empty() {
    let v: SmallVec<i32, 5> = SmallVec::from_iter_items([] as [i32; 0]);
    assert_eq!(v.len(), 0);
    assert_eq!(v.capacity(), 5);
}

#[test]
fn initializer_list_fits_inline() {
    let v: SmallVec<i32, 5> = SmallVec::from_iter_items([1, 2, 3]);
    assert_eq!(v.len(), 3);
    assert_eq!(v.capacity(), 5);
    assert_eq!(v[0], 1);
    assert_eq!(v[1], 2);
    assert_eq!(v[2], 3);
}

#[test]
fn initializer_list_exceeds_inline() {
    let v: SmallVec<i32, 5> = SmallVec::from_iter_items([6, 5, 4, 3, 2, 1]);
    assert_eq!(v.len(), 6);
    assert!(v.capacity() >= 6);
    assert_eq!(v[0], 6);
    assert_eq!(v[5], 1);
}

// ---------------------------------------------------------------------------
// No local storage (N=0)
// ---------------------------------------------------------------------------

#[test]
fn no_local_storage_starts_empty() {
    let v: SmallVec<i32, 0> = SmallVec::new();
    assert_eq!(v.len(), 0);
    assert_eq!(v.capacity(), 0);
}

#[test]
fn no_local_storage_push_back() {
    let mut v: SmallVec<i32, 0> = SmallVec::new();
    v.push(1337);
    assert_eq!(v.len(), 1);
    assert!(v.capacity() >= 1);
    assert_eq!(v[0], 1337);

    v.push(1338);
    assert_eq!(v.len(), 2);
    assert_eq!(v[0], 1337);
    assert_eq!(v[1], 1338);

    v.push(1339);
    assert_eq!(v.len(), 3);
    assert_eq!(v[0], 1337);
    assert_eq!(v[2], 1339);
}

#[test]
fn no_local_storage_insert_front() {
    let mut v: SmallVec<i32, 0> = SmallVec::new();
    v.push(1337);
    v.push(1338);
    v.push(1339);
    v.insert(0, 1313);
    assert_eq!(v.len(), 4);
    assert_eq!(v[0], 1313);
    assert_eq!(v[3], 1339);
}

#[test]
fn no_local_storage_erase_range() {
    let mut v: SmallVec<i32, 0> = SmallVec::new();
    v.push(1313);
    v.push(1337);
    v.push(1338);
    v.push(1339);
    // Erase indices 1..3 (elements 1337, 1338) using remove twice.
    v.remove(1);
    v.remove(1);
    assert_eq!(v.len(), 2);
    assert_eq!(v[0], 1313);
    assert_eq!(v[1], 1339);
}

#[test]
fn no_local_storage_pop_back() {
    let mut v: SmallVec<i32, 0> = SmallVec::new();
    v.push(1313);
    v.push(1339);
    v.pop();
    assert_eq!(v.len(), 1);
    assert_eq!(v[0], 1313);
}

#[test]
fn no_local_storage_clear_keeps_capacity() {
    let mut v: SmallVec<i32, 0> = SmallVec::new();
    v.push(1);
    v.push(2);
    v.push(3);
    let cap_before = v.capacity();
    v.clear();
    assert_eq!(v.len(), 0);
    assert_eq!(v.capacity(), cap_before);
}

// ---------------------------------------------------------------------------
// Growth: inline -> heap transitions
// ---------------------------------------------------------------------------

#[test]
fn growth_push_inline_then_heap() {
    let mut v: SmallVec<i32, 2> = SmallVec::new();

    v.push(1);
    assert_eq!(v.len(), 1);
    assert_eq!(v.capacity(), 2);
    assert!(!v.is_heap());

    v.push(2);
    assert_eq!(v.len(), 2);
    assert_eq!(v.capacity(), 2);
    assert!(!v.is_heap());

    // Third push spills to heap.
    v.push(3);
    assert_eq!(v.len(), 3);
    assert!(v.capacity() >= 4);
    assert!(v.is_heap());
    assert_eq!(v[0], 1);
    assert_eq!(v[1], 2);
    assert_eq!(v[2], 3);
}

#[test]
fn growth_push_after_clear_stays_heap() {
    let mut v: SmallVec<i32, 2> = SmallVec::new();
    v.push(1);
    v.push(2);
    v.push(3); // triggers heap
    v.push(4);
    v.clear();
    assert_eq!(v.len(), 0);
    assert!(v.capacity() >= 4);

    v.push(5);
    assert_eq!(v.len(), 1);
    assert_eq!(v[0], 5);
}

#[test]
fn reserve_in_empty_vector() {
    let mut v: SmallVec<i32, 2> = SmallVec::new();
    assert_eq!(v.capacity(), 2);
    v.reserve(100);
    assert_eq!(v.len(), 0);
    assert!(v.capacity() >= 100);
}

// ---------------------------------------------------------------------------
// Iteration
// ---------------------------------------------------------------------------

#[test]
fn assign_from_iterator() {
    let source = vec![10, 20, 30, 40, 50, 60, 70, 80, 90, 100];
    let mut v: SmallVec<i32, 1> = SmallVec::new();
    for &x in &source {
        v.push(x);
    }
    assert_eq!(v.len(), source.len());
    for (i, &x) in v.iter().enumerate() {
        assert_eq!(x, source[i]);
    }
}

#[test]
fn indexing_operator() {
    let source = vec![10, 20, 30, 40, 50];
    let v: SmallVec<i32, 1> = SmallVec::from_iter_items(source.iter().copied());
    for i in 0..source.len() {
        assert_eq!(v[i], source[i]);
    }
}

#[test]
fn forward_iterator() {
    let source = vec![10, 20, 30, 40, 50];
    let v: SmallVec<i32, 1> = SmallVec::from_iter_items(source.iter().copied());
    let collected: Vec<_> = v.iter().copied().collect();
    assert_eq!(collected, source);
}

#[test]
fn equality_comparison() {
    let v1: SmallVec<i32, 1> = SmallVec::from_iter_items([10, 20, 30]);
    let v2 = v1.clone();
    assert_eq!(v1, v2);
}

#[test]
fn inequality_comparison() {
    let v1: SmallVec<i32, 1> = SmallVec::from_iter_items([10, 20, 30]);
    let v2: SmallVec<i32, 1> = SmallVec::new();
    assert_ne!(v1, v2);
}

// ---------------------------------------------------------------------------
// Copy into vector via bulk data (mirrors testCopyIntoVector)
// ---------------------------------------------------------------------------

fn check_copy_roundtrip<T: Clone + PartialEq + std::fmt::Debug>(data: &[T]) {
    // Inline-capacity variant
    let mut v1: SmallVec<T, 10> = SmallVec::new();
    for x in data {
        v1.push(x.clone());
    }
    for (i, x) in data.iter().enumerate() {
        assert_eq!(&v1[i], x, "inline mismatch at {i}");
    }

    // Heap-always variant (N=1, data len > 1)
    let mut v2: SmallVec<T, 1> = SmallVec::new();
    for x in data {
        v2.push(x.clone());
    }
    for (i, x) in data.iter().enumerate() {
        assert_eq!(&v2[i], x, "heap mismatch at {i}");
    }
}

#[test]
fn copy_into_vec_i32() {
    check_copy_roundtrip(&[0i32, 1, 5, 75]);
}

#[test]
fn copy_into_vec_f64() {
    check_copy_roundtrip(&[0.0f64, 1.0, 0.5, 0.75]);
}

#[test]
fn copy_into_vec_f32() {
    check_copy_roundtrip(&[0.0f32, 1.0, 0.5, 0.75]);
}

#[test]
fn copy_into_vec_usize() {
    check_copy_roundtrip(&[0usize, 1, 5, 75]);
}

#[test]
fn copy_into_vec_array2_i32() {
    check_copy_roundtrip(&[[0i32, 0], [1, 0], [0, 1]]);
}

#[test]
fn copy_into_vec_array3_i32() {
    check_copy_roundtrip(&[[0i32, 0, 0], [1, 0, 0], [0, 1, 0], [0, 0, 1]]);
}

#[test]
fn copy_into_vec_array2_f64() {
    check_copy_roundtrip(&[[0.0f64, 0.0], [1.0, 0.0], [0.0, 1.0]]);
}

// ---------------------------------------------------------------------------
// Insertion - trivial types
// ---------------------------------------------------------------------------

#[test]
fn insert_at_end_inline_has_room() {
    let source_a: Vec<i32> = (0..10).collect();

    let mut a: SmallVec<i32, 15> = SmallVec::new();
    for &x in &source_a {
        a.push(x);
    }
    for (i, &x) in a.iter().enumerate() {
        assert_eq!(x, source_a[i]);
    }
}

#[test]
fn insert_at_end_heap_has_room() {
    let source_a: Vec<i32> = (0..10).collect();

    let mut a: SmallVec<i32, 1> = SmallVec::with_capacity(15);
    for &x in &source_a {
        a.push(x);
    }
    for (i, &x) in a.iter().enumerate() {
        assert_eq!(x, source_a[i]);
    }
}

#[test]
fn insert_at_front_local_has_room() {
    let source_a: Vec<i32> = (0..10).collect();
    let source_b = vec![999i32, 998, 997, 996];

    let mut a: SmallVec<i32, 15> = SmallVec::from_iter_items(source_a.iter().copied());
    for (idx, &x) in source_b.iter().enumerate() {
        a.insert(idx, x);
    }

    assert_eq!(a[0], 999);
    assert_eq!(a[1], 998);
    assert_eq!(a[2], 997);
    assert_eq!(a[3], 996);
    assert_eq!(a[4], 0);
    assert_eq!(a[13], 9);
}

#[test]
fn bulk_insert_front_remote_has_room() {
    let source_a: Vec<i32> = (0..10).collect();
    let source_b = vec![999i32, 998, 997, 996];

    let mut a: SmallVec<i32, 1> = SmallVec::from_iter_items(source_a.iter().copied());
    for (idx, &x) in source_b.iter().enumerate() {
        a.insert(idx, x);
    }

    assert_eq!(a[0], 999);
    assert_eq!(a[1], 998);
    assert_eq!(a[2], 997);
    assert_eq!(a[3], 996);
    assert_eq!(a[4], 0);
    assert_eq!(a[13], 9);
}

#[test]
fn middle_insertion_local_has_room() {
    let source_a: Vec<i32> = (0..10).collect();
    let source_b = vec![999i32, 998, 997, 996];

    let mut a: SmallVec<i32, 15> = SmallVec::from_iter_items(source_a.iter().copied());
    for (offset, &x) in source_b.iter().enumerate() {
        a.insert(2 + offset, x);
    }

    assert_eq!(a[0], 0);
    assert_eq!(a[1], 1);
    assert_eq!(a[2], 999);
    assert_eq!(a[3], 998);
    assert_eq!(a[4], 997);
    assert_eq!(a[5], 996);
    assert_eq!(a[6], 2);
    assert_eq!(a[13], 9);
}

#[test]
fn repeated_insertions_capacity_bound() {
    // Mirrors the PRES-70771 regression test: 2048 single-element pushes at
    // the end must not cause runaway allocation.
    const NUM_INSERTIONS: usize = 2048;
    let mut a: SmallVec<i32, 1> = SmallVec::new();
    for i in 0..NUM_INSERTIONS {
        a.push(1);
        assert!(
            a.capacity() <= 4 * (i + 1),
            "capacity {} too large after {} insertions",
            a.capacity(),
            i + 1
        );
    }
    assert_eq!(a.len(), NUM_INSERTIONS);
}

// ---------------------------------------------------------------------------
// Resize
// ---------------------------------------------------------------------------

#[test]
fn resize_shrink_trivial() {
    let source: Vec<i32> = (0..100).collect();
    let mut v: SmallVec<i32, 10> = SmallVec::from_iter_items(source.iter().copied());
    assert_eq!(v.len(), 100);

    v.resize(73, 0);
    assert_eq!(v.len(), 73);
    assert!(v.capacity() >= 73);
}

#[test]
fn resize_grow_trivial() {
    let source: Vec<i32> = (0..100).collect();
    let mut v: SmallVec<i32, 10> = SmallVec::from_iter_items(source.iter().copied());
    v.resize(5, 17);
    assert_eq!(v.len(), 5);
    v.resize(150, 17);
    assert_eq!(v.len(), 150);
    // Elements 5..150 must be 17.
    for i in 5..150 {
        assert_eq!(v[i], 17, "unexpected value at index {i}");
    }
}

// ---------------------------------------------------------------------------
// Erase with expected element ordering (mirrors testErase in C++)
// SmallVec uses remove(index) for single-element erase.
// Range erase: remove elements in a loop from start to end-1.
// ---------------------------------------------------------------------------

/// Helper: remove elements [start, end) preserving order, return removed slice copy.
fn erase_range(v: &mut SmallVec<String, 1>, start: usize, end: usize) -> Vec<String> {
    let removed: Vec<String> = v.as_slice()[start..end].to_vec();
    for _ in start..end {
        v.remove(start);
    }
    removed
}

#[test]
fn erase_from_front_correct_elements() {
    let mut vec: SmallVec<String, 1> =
        SmallVec::from_iter_items(["0", "1", "2", "3", "4", "5"].map(|s| s.to_string()));

    let removed = erase_range(&mut vec, 0, 2);
    assert_eq!(removed, vec!["0", "1"]);
    assert_eq!(vec[0], "2");
    assert_eq!(vec.len(), 4);
}

#[test]
fn erase_from_middle_correct_elements() {
    let mut vec: SmallVec<String, 1> =
        SmallVec::from_iter_items(["0", "1", "2", "3", "4", "5"].map(|s| s.to_string()));

    let removed = erase_range(&mut vec, 2, 4);
    assert_eq!(removed, vec!["2", "3"]);
    assert_eq!(vec[2], "4");
    assert_eq!(vec.len(), 4);
}

#[test]
fn erase_up_to_end() {
    let mut vec: SmallVec<String, 1> =
        SmallVec::from_iter_items(["0", "1", "2", "3", "4", "5"].map(|s| s.to_string()));

    erase_range(&mut vec, 3, 6);
    assert_eq!(vec.len(), 3);
}

#[test]
fn erase_sort_unique_dedup() {
    // Mirrors the sort+unique+erase pattern from C++.
    let mut vec: SmallVec<String, 1> = SmallVec::from_iter_items(
        [
            "asdf", "fdas", "qwer", "asdf", "zxcv", "fdas", "zxcv", "qwer", "zxcv", "123", "9087",
            "123",
        ]
        .map(|s| s.to_string()),
    );

    // Sort in-place via the Deref<Target=[T]> impl.
    vec.sort();

    // Manual dedup: remove consecutive duplicates.
    let mut i = 1;
    while i < vec.len() {
        if vec[i] == vec[i - 1] {
            vec.remove(i);
        } else {
            i += 1;
        }
    }

    assert_eq!(vec[0], "123");
    assert_eq!(vec[1], "9087");
    assert_eq!(vec[2], "asdf");
    assert_eq!(vec[3], "fdas");
    assert_eq!(vec[4], "qwer");
    assert_eq!(vec[5], "zxcv");
}
