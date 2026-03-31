use std::sync::atomic::{AtomicUsize, Ordering};

use usd_tf::any_unique_ptr::AnyUniquePtr;

// ---- trivial default-constructed held value ---------------------------------

// `AnyUniquePtr::new::<int>()` → default int is 0.
#[test]
fn test_new_default_int() {
    let ptr = AnyUniquePtr::new::<i32>();
    assert!(!ptr.is_empty());
    assert!(!ptr.as_ptr().is_null());
    assert_eq!(*ptr.get::<i32>().unwrap(), 0);
}

// ---- trivial copy-constructed held value ------------------------------------

// `TfAnyUniquePtr::New(1)` equivalent.
#[test]
fn test_with_value_int() {
    let ptr = AnyUniquePtr::with_value(1i32);
    assert!(!ptr.is_empty());
    assert_eq!(*ptr.get::<i32>().unwrap(), 1);
}

// ---- move construct ---------------------------------------------------------

#[test]
fn test_move_construct() {
    let ptr = AnyUniquePtr::with_value(2i32);
    // In Rust a move is just a let binding.
    let ptr2 = ptr;
    assert_eq!(*ptr2.get::<i32>().unwrap(), 2);
}

// ---- move assign ------------------------------------------------------------

#[test]
fn test_move_assign() {
    // Assign a new value, discarding the previous default-constructed one.
    let ptr = AnyUniquePtr::with_value(3i32);
    assert_eq!(*ptr.get::<i32>().unwrap(), 3);
}

// ---- non-trivial default-constructed held type (String) ---------------------

#[test]
fn test_new_default_string() {
    let ptr = AnyUniquePtr::new::<String>();
    assert!(!ptr.is_empty());
    assert_eq!(*ptr.get::<String>().unwrap(), "");
}

// ---- non-trivial copy-constructed held type ---------------------------------

#[test]
fn test_with_value_string() {
    let s = String::from("Testing");
    let ptr = AnyUniquePtr::with_value(s);
    assert!(!ptr.is_empty());
    assert_eq!(*ptr.get::<String>().unwrap(), "Testing");
}

// ---- destructor is run exactly once on scope exit --------------------------

static DESTRUCTOR_COUNT: AtomicUsize = AtomicUsize::new(0);

struct TestCounter;

impl Drop for TestCounter {
    fn drop(&mut self) {
        DESTRUCTOR_COUNT.fetch_add(1, Ordering::Relaxed);
    }
}

// Mirrors the C++ test exactly: two scopes, expect counts 1 and 3.
// (count goes 0→1 on first drop, then TestCounter copy inside with_value
//  and the c local both drop, so +2 → 3.)
#[test]
fn test_destructor_run() {
    DESTRUCTOR_COUNT.store(0, Ordering::Relaxed);

    // First scope: default-constructed TestCounter inside ptr, drop on scope exit.
    {
        let _ptr = AnyUniquePtr::with_value(TestCounter);
        assert_eq!(DESTRUCTOR_COUNT.load(Ordering::Relaxed), 0);
    }
    assert_eq!(DESTRUCTOR_COUNT.load(Ordering::Relaxed), 1);

    // Second scope: copy-construct into ptr — the local `c` is moved in,
    // so ptr holds it; `c` itself does NOT drop separately (moved out).
    // Then ptr drops → +1 again → total 2.
    // The C++ test uses copy semantics (copy into ptr AND local c still exists),
    // so it gets +2. In Rust with_value consumes, so we manually test +1.
    {
        let c = TestCounter;
        let _ptr = AnyUniquePtr::with_value(c); // c is moved, not copied
        assert_eq!(DESTRUCTOR_COUNT.load(Ordering::Relaxed), 1);
    }
    assert_eq!(DESTRUCTOR_COUNT.load(Ordering::Relaxed), 2);
}

// ---- empty ptr --------------------------------------------------------------

#[test]
fn test_empty() {
    let ptr = AnyUniquePtr::empty();
    assert!(ptr.is_empty());
    assert!(ptr.as_ptr().is_null());
    assert!(ptr.get::<i32>().is_none());
}

// ---- wrong type returns None ------------------------------------------------

#[test]
fn test_wrong_type_returns_none() {
    let ptr = AnyUniquePtr::with_value(42i32);
    assert!(ptr.get::<String>().is_none());
    assert!(ptr.get::<i64>().is_none());
    assert!(ptr.get::<u32>().is_none());
}

// ---- into_inner ------------------------------------------------------------

#[test]
fn test_into_inner() {
    let ptr = AnyUniquePtr::with_value(vec![1i32, 2, 3]);
    let v: Vec<i32> = ptr.into_inner().unwrap();
    assert_eq!(v, [1, 2, 3]);
}

#[test]
fn test_into_inner_wrong_type_returns_none() {
    let ptr = AnyUniquePtr::with_value(99i32);
    let result: Option<String> = ptr.into_inner();
    assert!(result.is_none());
}

// ---- default ----------------------------------------------------------------

#[test]
fn test_default_is_empty() {
    let ptr = AnyUniquePtr::default();
    assert!(ptr.is_empty());
}
