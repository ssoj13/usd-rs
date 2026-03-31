/// Integration tests for stl.rs - port of testenv/stl.cpp
///
/// Tests map_lookup, map_lookup_ptr, ordered_pair, ordered_set_difference,
/// ordered_uniquing_set_difference, and TfGet (TupleGet) equivalents.
use std::collections::HashMap;
use usd_tf::stl::{
    map_lookup, map_lookup_ptr, ordered_pair, ordered_set_difference,
    ordered_uniquing_set_difference,
};

// ---------------------------------------------------------------------------
// testSetDifferences
// ---------------------------------------------------------------------------

#[test]
fn ordered_set_difference_basic() {
    // C++: v1 = [1,3,3,1], v2 = [2,3,2], expected = [1,3,1]
    let v1 = vec![1i32, 3, 3, 1];
    let v2 = vec![2i32, 3, 2];
    let expected = vec![1i32, 3, 1];

    let result: Vec<i32> = ordered_set_difference(v1.iter(), v2.iter())
        .copied()
        .collect();
    assert_eq!(result, expected);
}

#[test]
fn ordered_uniquing_set_difference_basic() {
    // C++: v1 = [1,3,3,1], v2 = [2,3,2], expected = [1]
    let v1 = vec![1i32, 3, 3, 1];
    let v2 = vec![2i32, 3, 2];
    let expected = vec![1i32];

    let result: Vec<i32> = ordered_uniquing_set_difference(v1.iter(), v2.iter())
        .copied()
        .collect();
    assert_eq!(result, expected);
}

#[test]
fn ordered_set_difference_empty_result() {
    let v1 = vec![1i32, 2, 3];
    let v2 = vec![1i32, 2, 3, 4, 5];
    let result: Vec<i32> = ordered_set_difference(v1.iter(), v2.iter())
        .copied()
        .collect();
    assert!(result.is_empty());
}

#[test]
fn ordered_set_difference_nothing_removed() {
    let v1 = vec![1i32, 2, 3];
    let v2 = vec![4i32, 5, 6];
    let result: Vec<i32> = ordered_set_difference(v1.iter(), v2.iter())
        .copied()
        .collect();
    assert_eq!(result, vec![1, 2, 3]);
}

// ---------------------------------------------------------------------------
// testGetPair / testGetTuple - via TupleGet trait
// ---------------------------------------------------------------------------

#[test]
fn tuple_get_pair_element_0() {
    use usd_tf::stl::TupleGet;
    let pair: (i32, String) = (1, "A".to_string());
    assert_eq!(*TupleGet::<0>::get(&pair), 1);
}

#[test]
fn tuple_get_pair_element_1() {
    use usd_tf::stl::TupleGet;
    let pair: (i32, String) = (1, "A".to_string());
    assert_eq!(*TupleGet::<1>::get(&pair), "A");
}

#[test]
fn tuple_get_const_pair_element_0() {
    use usd_tf::stl::TupleGet;
    let pair: (i32, String) = (2, "B".to_string());
    assert_eq!(*TupleGet::<0>::get(&pair), 2);
}

#[test]
fn tuple_get_const_pair_element_1() {
    use usd_tf::stl::TupleGet;
    let pair: (i32, String) = (2, "B".to_string());
    assert_eq!(*TupleGet::<1>::get(&pair), "B");
}

#[test]
fn get_first_transform() {
    use usd_tf::stl::get_first;
    let pairs = vec![(1i32, "A"), (2, "B"), (3, "C"), (4, "D")];
    let firsts: Vec<i32> = pairs.iter().map(get_first).copied().collect();
    assert_eq!(firsts, vec![1, 2, 3, 4]);
}

#[test]
fn get_second_transform() {
    use usd_tf::stl::get_second;
    let pairs = vec![(1i32, "A"), (2, "B"), (3, "C"), (4, "D")];
    let seconds: Vec<&&str> = pairs.iter().map(get_second).collect();
    assert_eq!(seconds, vec![&"A", &"B", &"C", &"D"]);
}

// ---------------------------------------------------------------------------
// TfMapLookup / TfMapLookupPtr via map_lookup / map_lookup_ptr
// ---------------------------------------------------------------------------

#[test]
fn map_lookup_existing_key() {
    let mut m: HashMap<String, i32> = HashMap::new();
    m.insert("key".to_string(), 1);

    let result = map_lookup(&m, "key");
    assert_eq!(result, Some(&1));
}

#[test]
fn map_lookup_missing_key() {
    let mut m: HashMap<String, i32> = HashMap::new();
    m.insert("key".to_string(), 1);

    let result = map_lookup(&m, "blah");
    assert_eq!(result, None);
}

#[test]
fn map_lookup_ptr_existing_key() {
    let mut m: HashMap<String, i32> = HashMap::new();
    m.insert("key".to_string(), 1);

    let ptr = map_lookup_ptr(&m, "key");
    assert!(ptr.is_some());
    // Pointer identity: must point into the map
    assert_eq!(ptr.unwrap(), m.get("key").unwrap());
}

#[test]
fn map_lookup_ptr_missing_key() {
    let mut m: HashMap<String, i32> = HashMap::new();
    m.insert("key".to_string(), 1);

    let ptr = map_lookup_ptr(&m, "blah");
    assert!(ptr.is_none());
}

#[test]
fn map_lookup_via_hashmap() {
    // Mirrors the TfHashMap variant in C++: same semantics as std HashMap.
    use usd_tf::hash::TfHash;
    let mut hm: HashMap<String, i32, TfHash> = HashMap::with_hasher(TfHash);
    hm.insert("key".to_string(), 1);

    assert_eq!(hm.get("key"), Some(&1));
    assert_eq!(hm.get("blah"), None);
}

// ---------------------------------------------------------------------------
// TfOrderedPair
// ---------------------------------------------------------------------------

#[test]
fn ordered_pair_already_ordered() {
    assert_eq!(ordered_pair(1i32, 2), (1, 2));
}

#[test]
fn ordered_pair_reversed() {
    // C++: TfOrderedPair(2, 1) == pair<int,int>(1,2)
    assert_eq!(ordered_pair(2i32, 1), (1, 2));
}

#[test]
fn ordered_pair_symmetry() {
    // C++: TfOrderedPair(1,2) == TfOrderedPair(2,1)
    assert_eq!(ordered_pair(1i32, 2), ordered_pair(2i32, 1));
}

#[test]
fn ordered_pair_equal_elements() {
    assert_eq!(ordered_pair(5i32, 5), (5, 5));
}

#[test]
fn ordered_pair_strings() {
    assert_eq!(ordered_pair("b", "a"), ("a", "b"));
    assert_eq!(ordered_pair("a", "b"), ("a", "b"));
}
