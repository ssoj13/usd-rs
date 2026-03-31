use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use usd_tf::{RefPtr, WeakPtr};

fn hash_of<T: Hash>(v: &T) -> u64 {
    let mut h = DefaultHasher::new();
    v.hash(&mut h);
    h.finish()
}

// ---- basic liveness ---------------------------------------------------------

// A live weak ptr should upgrade successfully.
#[test]
fn test_live_upgrade() {
    let strong = RefPtr::new(42i32);
    let weak = WeakPtr::from_ref(&strong);

    assert!(!weak.is_expired());
    assert!(weak.is_valid());
    let upgraded = weak.upgrade();
    assert!(upgraded.is_some());
    assert_eq!(*upgraded.unwrap(), 42);
}

// After the last strong ref is dropped, upgrade returns None.
#[test]
fn test_expired_after_drop() {
    let weak;
    {
        let strong = RefPtr::new(99i32);
        weak = WeakPtr::from_ref(&strong);
        assert!(!weak.is_expired());
    }
    assert!(weak.is_expired());
    assert!(weak.upgrade().is_none());
}

// Two weak ptrs to the same object compare equal.
#[test]
fn test_two_weak_ptrs_same_object_are_equal() {
    let strong = RefPtr::new(1i32);
    let w1 = WeakPtr::from_ref(&strong);
    let w2 = WeakPtr::from_ref(&strong);

    assert_eq!(w1, w2);
    assert!(WeakPtr::ptr_eq(&w1, &w2));
}

// Weak ptrs to different objects are unequal.
#[test]
fn test_weak_ptrs_different_objects_are_unequal() {
    let s1 = RefPtr::new(1i32);
    let s2 = RefPtr::new(1i32);
    let w1 = WeakPtr::from_ref(&s1);
    let w2 = WeakPtr::from_ref(&s2);

    assert_ne!(w1, w2);
    assert!(!WeakPtr::ptr_eq(&w1, &w2));
}

// Both expired ptrs that pointed to the same object compare equal.
#[test]
fn test_expired_ptrs_same_object_still_equal() {
    let w1;
    let w2;
    {
        let strong = RefPtr::new(0i32);
        w1 = WeakPtr::from_ref(&strong);
        w2 = WeakPtr::from_ref(&strong);
    }
    assert!(w1.is_expired());
    assert!(w2.is_expired());
    assert_eq!(w1, w2);
}

// ---- clone ------------------------------------------------------------------

#[test]
fn test_clone_live() {
    let strong = RefPtr::new(7i32);
    let w1 = WeakPtr::from_ref(&strong);
    let w2 = w1.clone();

    assert!(!w1.is_expired());
    assert!(!w2.is_expired());
    assert!(WeakPtr::ptr_eq(&w1, &w2));
    assert_eq!(w1, w2);
}

#[test]
fn test_clone_null_stays_not_invalid() {
    let null_weak: WeakPtr<i32> = WeakPtr::new();
    let cloned = null_weak.clone();
    assert!(!cloned.is_invalid());
    assert!(cloned.is_expired());
}

// ---- IsInvalid / C++ semantics ----------------------------------------------

// Null-constructed weak ptr: was never associated with any object → NOT invalid.
#[test]
fn test_is_invalid_null_constructed() {
    let weak: WeakPtr<i32> = WeakPtr::new();
    assert!(!weak.is_invalid());
    assert!(weak.is_expired()); // upgrade fails but it is not "invalid"
}

// After setting to null (like C++ `lPtr = TfNullPtr`), is_invalid() is false.
#[test]
fn test_is_invalid_after_reset_to_null() {
    let strong = RefPtr::new(0i32);
    let mut weak = WeakPtr::from_ref(&strong);
    drop(strong);

    // At this point the expired weak is invalid
    assert!(weak.is_invalid());
    assert!(!weak.is_valid());

    // Reassign to null
    weak = WeakPtr::new();
    assert!(!weak.is_invalid()); // null is NOT invalid
    assert!(!weak.is_valid()); // but also not valid
}

// Live pointer: is_invalid() is false; after drop it becomes true.
#[test]
fn test_is_invalid_transitions() {
    let strong = RefPtr::new(42i32);
    let weak = WeakPtr::from_ref(&strong);

    assert!(!weak.is_invalid());
    assert!(!weak.is_expired());

    drop(strong);

    assert!(weak.is_invalid());
    assert!(weak.is_expired());
}

// Cloned live pointer inherits from_live flag; both expire together.
#[test]
fn test_is_invalid_clone_propagates() {
    let strong = RefPtr::new(5i32);
    let w1 = WeakPtr::from_ref(&strong);
    let w2 = w1.clone();

    drop(strong);

    assert!(w1.is_invalid());
    assert!(w2.is_invalid());
}

// Default is equivalent to null-constructed.
#[test]
fn test_is_invalid_default() {
    let weak: WeakPtr<i32> = WeakPtr::default();
    assert!(!weak.is_invalid());
}

// ---- strong_count -----------------------------------------------------------

#[test]
fn test_strong_count_tracks_refs() {
    let s1 = RefPtr::new(0i32);
    let weak = WeakPtr::from_ref(&s1);

    assert_eq!(weak.strong_count(), 1);

    let s2 = s1.clone();
    assert_eq!(weak.strong_count(), 2);

    drop(s2);
    assert_eq!(weak.strong_count(), 1);

    drop(s1);
    assert_eq!(weak.strong_count(), 0);
}

// ---- hash -------------------------------------------------------------------

// Null weak ptrs hash the same (both point to "nothing").
#[test]
fn test_hash_null_equivalence() {
    let w1: WeakPtr<i32> = WeakPtr::new();
    let w2: WeakPtr<i32> = WeakPtr::new();
    assert_eq!(hash_of(&w1), hash_of(&w2));
}

// Clone produces identical hash.
#[test]
fn test_hash_same_for_clones() {
    let strong = RefPtr::new(3i32);
    let w1 = WeakPtr::from_ref(&strong);
    let w2 = w1.clone();
    assert_eq!(hash_of(&w1), hash_of(&w2));
}

// Hash-set deduplication: ptr + clone = 1 entry.
#[test]
fn test_hash_set_dedup() {
    use std::collections::HashSet;

    let strong = RefPtr::new(0i32);
    let w1 = WeakPtr::from_ref(&strong);
    let w2 = w1.clone();

    let mut set = HashSet::new();
    set.insert(w1);
    set.insert(w2);
    assert_eq!(set.len(), 1);
}

// ---- ordering (comparisons, C++ _TestComparisons equivalent) ----------------

#[test]
fn test_comparisons_live_vs_null() {
    let strong = RefPtr::new(0i32);
    let w = WeakPtr::from_ref(&strong);

    // A live weak ptr is never equal to a null one.
    let null_w: WeakPtr<i32> = WeakPtr::new();
    assert_ne!(w, null_w);
}

#[test]
fn test_address_ordering_between_two_live_ptrs() {
    let s1 = RefPtr::new(0i32);
    let s2 = RefPtr::new(0i32);
    let w1 = WeakPtr::from_ref(&s1);
    let w2 = WeakPtr::from_ref(&s2);

    // WeakPtr doesn't implement PartialOrd; compare raw addresses directly,
    // which is what C++ weak ptr ordering delegates to.
    let p1 = w1.as_ptr();
    let p2 = w2.as_ptr();
    assert!(
        p1 < p2 || p2 < p1,
        "different allocations must have different addresses"
    );

    // Reflexive: ptr == itself through ptr_eq.
    assert_eq!(w1, w1.clone());
}

// ---- create_weak_ptr convenience function -----------------------------------

#[test]
fn test_create_weak_ptr_fn() {
    use usd_tf::create_weak_ptr;

    let strong = RefPtr::new(10i32);
    let weak = create_weak_ptr(&strong);

    assert!(!weak.is_expired());
    assert_eq!(*weak.upgrade().unwrap(), 10);
}

// ---- From conversions -------------------------------------------------------

#[test]
fn test_from_ref_ptr_ref() {
    let strong = RefPtr::new(20i32);
    let weak: WeakPtr<i32> = WeakPtr::from(&strong);

    assert!(!weak.is_expired());
    assert_eq!(*weak.upgrade().unwrap(), 20);
}
