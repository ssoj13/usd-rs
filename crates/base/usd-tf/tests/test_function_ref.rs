use usd_tf::function_ref::FunctionRef;

// C++ test creates two FunctionRefs from two lambdas and verifies calls.
#[test]
fn test_basic_call() {
    let lambda1 = |arg: i32| arg + 1;
    let lambda2 = |arg: i32| arg + 2;

    let f1 = FunctionRef::new(&lambda1 as &dyn Fn(i32) -> i32);
    let f2 = FunctionRef::new(&lambda2 as &dyn Fn(i32) -> i32);

    // C++: TF_AXIOM(lambda1(1) == f1(1))
    assert_eq!(lambda1(1), f1.call((1,)));
    // C++: TF_AXIOM(lambda2(1) == f2(1))
    assert_eq!(lambda2(1), f2.call((1,)));
    // C++: TF_AXIOM(lambda1(1) != f2(1))
    assert_ne!(lambda1(1), f2.call((1,)));
    // C++: TF_AXIOM(lambda2(1) != f1(1))
    assert_ne!(lambda2(1), f1.call((1,)));
}

// C++ test swaps f1 and f2, then verifies references point to the original lambdas.
// In Rust FunctionRef is Copy; swap is modelled by reordering the bindings.
#[test]
fn test_swap_semantics() {
    let lambda1 = |arg: i32| arg + 1;
    let lambda2 = |arg: i32| arg + 2;

    let f1 = FunctionRef::new(&lambda1 as &dyn Fn(i32) -> i32);
    let f2 = FunctionRef::new(&lambda2 as &dyn Fn(i32) -> i32);

    // Simulate f1.swap(f2)
    let (f1, f2) = (f2, f1);

    // After swap: f1 calls lambda2, f2 calls lambda1.
    // C++: TF_AXIOM(lambda1(1) == f2(1))
    assert_eq!(lambda1(1), f2.call((1,)));
    // C++: TF_AXIOM(lambda2(1) == f1(1))
    assert_eq!(lambda2(1), f1.call((1,)));
    // C++: TF_AXIOM(lambda1(1) != f1(1))
    assert_ne!(lambda1(1), f1.call((1,)));
    // C++: TF_AXIOM(lambda2(1) != f2(1))
    assert_ne!(lambda2(1), f2.call((1,)));

    // Swap back (std::swap equivalent)
    let (f1, f2) = (f2, f1);

    // C++: TF_AXIOM(lambda1(1) == f1(1))
    assert_eq!(lambda1(1), f1.call((1,)));
    // C++: TF_AXIOM(lambda2(1) == f2(1))
    assert_eq!(lambda2(1), f2.call((1,)));
    // C++: TF_AXIOM(lambda1(1) != f2(1))
    assert_ne!(lambda1(1), f2.call((1,)));
    // C++: TF_AXIOM(lambda2(1) != f1(1))
    assert_ne!(lambda2(1), f1.call((1,)));
}

// C++ test: f2 = f1 (copy assign), both now call lambda1.
#[test]
fn test_copy_assign() {
    let lambda1 = |arg: i32| arg + 1;
    let lambda2 = |arg: i32| arg + 2;

    let f1 = FunctionRef::new(&lambda1 as &dyn Fn(i32) -> i32);
    let f2_initial = FunctionRef::new(&lambda2 as &dyn Fn(i32) -> i32);

    // C++: f2 = f1  — rebind to f1's target
    let f2 = f1;
    let _ = f2_initial; // suppress unused warning

    // C++: TF_AXIOM(f2(1) == f1(1))
    assert_eq!(f2.call((1,)), f1.call((1,)));
}

// C++ test: f2 = lambda3 (assign a new callable).
#[test]
fn test_assign_new_callable() {
    let lambda3 = |arg: i32| arg + 3;
    let f2 = FunctionRef::new(&lambda3 as &dyn Fn(i32) -> i32);

    // C++: TF_AXIOM(lambda3(1) == f2(1))
    assert_eq!(lambda3(1), f2.call((1,)));
}

// C++ test: copy-construct f3 from f2.
#[test]
fn test_copy_construct() {
    let lambda3 = |arg: i32| arg + 3;
    let f2 = FunctionRef::new(&lambda3 as &dyn Fn(i32) -> i32);

    let f3 = f2;

    // C++: TF_AXIOM(f3(1) == f2(1))
    assert_eq!(f3.call((1,)), f2.call((1,)));
}

// C++ test: copy-constructed ref must refer to the ORIGINAL callable, not wrap
// the FunctionRef itself.  After copy, reassigning the original must not affect
// the copy.
#[test]
fn test_copy_refers_to_original_callable() {
    let ok = || {};
    // Wrap in a plain fn pointer to avoid the `! -> !` vs `() -> ()` mismatch
    // that arises when coercing a `|| panic!(...)` literal to `&dyn Fn()`.
    let error: fn() = || panic!("copy should refer to original callable, not the ref wrapper");

    let reference = FunctionRef::new(&ok as &dyn Fn());
    // Copy-construct.
    let reference_copy = reference;

    // Rebind `reference` to the error function — copy must be unaffected.
    let _reference = FunctionRef::new(&error as &dyn Fn());

    // Must call ok, not error.
    reference_copy.call(());
}

// C++ test: copy-assigned ref must refer to the ORIGINAL callable.
#[test]
fn test_copy_assign_refers_to_original_callable() {
    let ok = || {};
    let error1: fn() = || panic!("assignment failed");
    let error2: fn() = || panic!("copy should refer to original callable, not the ref wrapper");

    let reference = FunctionRef::new(&ok as &dyn Fn());
    // Starts as error1 but is intentionally overwritten by copy-assign below.
    #[allow(unused_assignments)]
    let mut reference_copy = FunctionRef::new(&error1 as &dyn Fn());

    // Copy-assign.
    reference_copy = reference;

    // Rebind `reference` — the copy must still call ok.
    let _reference = FunctionRef::new(&error2 as &dyn Fn());

    // Must call ok, not error2.
    reference_copy.call(());
}
