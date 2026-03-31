// Port of testenv/iterator.cpp
//
// C++ original tests:
//   TestNonConst   — TfIterator over mutable vector<int> and map<string,char>
//   TestConst      — same with const containers
//   TestRefsAndTempsForAll — TF_FOR_ALL / TF_REVERSE_FOR_ALL over references
//   TestPointerIterators   — TF_FOR_ALL over TfSpan (raw-pointer iterators)

use std::collections::BTreeMap;

use usd_tf::iterator::{TfIterator, TfReverseIterator, make_iterator, make_reverse_iterator};
use usd_tf::{tf_for_all, tf_reverse_for_all};

// ---------------------------------------------------------------------------
// TestNonConst — iterate a mutable Vec<i32>
// ---------------------------------------------------------------------------

#[test]
fn non_const_vec_iteration() {
    let orig_vec: Vec<i32> = vec![0, -5, 5];
    let mut copy_vec: Vec<i32> = Vec::new();

    // Mirrors:  TfIterator<vector<int>> vecIter; for(vecIter = origVec; vecIter; ++vecIter)
    let mut iter = TfIterator::new(&orig_vec);
    while iter.is_valid() {
        copy_vec.push(*iter.get().unwrap());
        iter.advance();
    }
    // !vecIter after loop
    assert!(!iter.is_valid());
    assert_eq!(orig_vec, copy_vec);
}

#[test]
fn non_const_map_iteration() {
    let mut orig_map: BTreeMap<String, char> = BTreeMap::new();
    orig_map.insert("a".to_string(), 'a');
    orig_map.insert("b".to_string(), 'b');
    orig_map.insert("c".to_string(), 'c');

    let mut copy_map: BTreeMap<String, char> = BTreeMap::new();

    // Mirrors: TfIterator<map<string,char>> mapIter(origMap.begin(), origMap.end())
    let mut map_iter = TfIterator::wrap(orig_map.iter());
    // mapIterCopy = mapIter — we take a snapshot of a fresh iterator
    let map_iter_copy = TfIterator::wrap(orig_map.iter());

    // The two fresh iterators point to the same first element.
    assert_eq!(
        map_iter.get_ref().map(|(k, _)| k.as_str()),
        map_iter_copy.get_ref().map(|(k, _)| k.as_str()),
        "copy must start at same position as original"
    );

    while map_iter.is_valid() {
        let (k, v) = map_iter.get_ref().unwrap();
        copy_map.insert(k.to_string(), **v);
        map_iter.advance();
    }

    // mapIter is now exhausted — mapEnd equivalent
    assert!(!map_iter.is_valid());
    assert_eq!(orig_map, copy_map);

    // Iterate again from the copy
    copy_map.clear();
    let mut map_iter_copy2 = TfIterator::wrap(orig_map.iter());
    while map_iter_copy2.is_valid() {
        let (k, v) = map_iter_copy2.get_ref().unwrap();
        copy_map.insert(k.to_string(), **v);
        map_iter_copy2.advance();
    }
    assert_eq!(orig_map, copy_map);
}

// ---------------------------------------------------------------------------
// TestConst — same logic over immutable / shared references
// ---------------------------------------------------------------------------

#[test]
fn const_vec_iteration() {
    let orig_vec: Vec<i32> = vec![0, -5, 5];
    let mut copy_vec: Vec<i32> = Vec::new();

    let mut iter = TfIterator::new(&orig_vec);
    while iter.is_valid() {
        copy_vec.push(*iter.get().unwrap());
        iter.advance();
    }
    assert!(!iter.is_valid());
    assert_eq!(orig_vec, copy_vec);
}

#[test]
fn const_map_iteration() {
    let mut orig_map: BTreeMap<String, char> = BTreeMap::new();
    orig_map.insert("a".to_string(), 'a');
    orig_map.insert("b".to_string(), 'b');
    orig_map.insert("c".to_string(), 'c');

    let mut copy_map: BTreeMap<String, char> = BTreeMap::new();

    let mut iter = TfIterator::wrap(orig_map.iter());
    while iter.is_valid() {
        let (k, v) = iter.get_ref().unwrap();
        copy_map.insert(k.to_string(), **v);
        iter.advance();
    }
    assert_eq!(orig_map, copy_map);
}

// ---------------------------------------------------------------------------
// TestRefsAndTempsForAll — TF_FOR_ALL / TF_REVERSE_FOR_ALL over references
// ---------------------------------------------------------------------------

fn get_const_ref() -> &'static Vec<i32> {
    static DATA: std::sync::OnceLock<Vec<i32>> = std::sync::OnceLock::new();
    DATA.get_or_init(|| vec![3, 2, 1])
}

/// Mirrors: TF_FOR_ALL(i, GetConstRef()) { TF_AXIOM(*i == count--); }
#[test]
fn tf_for_all_const_ref_forward() {
    let data = get_const_ref();
    let mut count = 3_i32;
    tf_for_all!(i, *data, {
        assert_eq!(
            *i, count,
            "forward iteration: expected {} got {}",
            count, *i
        );
        count -= 1;
    });
    assert_eq!(count, 0);
}

/// Mirrors: TF_FOR_ALL(i, GetNonConstRef()) { TF_AXIOM(*i == count--); }
#[test]
fn tf_for_all_non_const_ref_forward() {
    let data: &Vec<i32> = &vec![3, 2, 1];
    let mut count = 3_i32;
    tf_for_all!(i, *data, {
        assert_eq!(*i, count);
        count -= 1;
    });
    assert_eq!(count, 0);
}

/// Mirrors: TF_REVERSE_FOR_ALL(i, GetConstRef()) { TF_AXIOM(*i == count++); }
#[test]
fn tf_reverse_for_all_const_ref() {
    let data = get_const_ref();
    let mut count = 1_i32;
    tf_reverse_for_all!(i, *data, {
        assert_eq!(
            *i, count,
            "reverse iteration: expected {} got {}",
            count, *i
        );
        count += 1;
    });
    assert_eq!(count, 4);
}

/// Mirrors: TF_REVERSE_FOR_ALL(i, GetNonConstRef()) { TF_AXIOM(*i == count++); }
#[test]
fn tf_reverse_for_all_non_const_ref() {
    let data: &Vec<i32> = &vec![3, 2, 1];
    let mut count = 1_i32;
    tf_reverse_for_all!(i, *data, {
        assert_eq!(*i, count);
        count += 1;
    });
    assert_eq!(count, 4);
}

// ---------------------------------------------------------------------------
// TestPointerIterators — TF_FOR_ALL over a span (raw-pointer iterators)
// ---------------------------------------------------------------------------

/// Mirrors the static int data[] / TfSpan<const int> test.
#[test]
fn tf_for_all_pointer_span() {
    static DATA: [i32; 3] = [3, 2, 1];
    let span: &[i32] = &DATA;

    let mut sum = 0_i32;
    let mut count = 0_usize;
    tf_for_all!(it, *span, {
        sum += it;
        count += 1;
    });

    assert_eq!(sum, 6, "sum of [3,2,1] must be 6");
    assert_eq!(count, 3, "count must be 3");
}

/// Mirrors the empty TfSpan<std::string> test.
#[test]
fn tf_for_all_empty_span() {
    let empty: &[String] = &[];

    let mut sum = 0_usize;
    let mut count = 0_usize;
    tf_for_all!(it, *empty, {
        sum += it.len();
        count += 1;
    });

    assert_eq!(sum, 0);
    assert_eq!(count, 0);
}

// ---------------------------------------------------------------------------
// Additional: TfReverseIterator (mirrors C++ TF_REVERSE_FOR_ALL internals)
// ---------------------------------------------------------------------------

#[test]
fn reverse_iterator_basic() {
    let data = vec![1, 2, 3];
    let mut iter = TfReverseIterator::new(&data);

    assert_eq!(*iter.get().unwrap(), 3);
    iter.advance();
    assert_eq!(*iter.get().unwrap(), 2);
    iter.advance();
    assert_eq!(*iter.get().unwrap(), 1);
    iter.advance();
    assert!(!iter.is_valid());
}

/// make_iterator / make_reverse_iterator convenience functions.
#[test]
fn make_iterator_helpers() {
    let data = vec![10, 20, 30];

    let iter = make_iterator(&data);
    assert_eq!(*iter.get().unwrap(), 10);

    let rev = make_reverse_iterator(&data);
    assert_eq!(*rev.get().unwrap(), 30);
}

/// Iterator implements std::iter::Iterator — can be collected.
#[test]
fn tf_iterator_as_std_iterator() {
    let data = vec![1, 2, 3];
    let collected: Vec<_> = TfIterator::new(&data).collect();
    assert_eq!(collected, vec![&1, &2, &3]);
}

/// Exhausted iterator's is_valid returns false, get returns None.
#[test]
fn iterator_exhausted_state() {
    let data: Vec<i32> = vec![];
    let iter = TfIterator::new(&data);
    assert!(!iter.is_valid());
    assert!(iter.get().is_none());
}
