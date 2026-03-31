// Port of testenv/scoped.cpp

use std::cell::Cell;
use usd_tf::scoped::{Scoped, ScopedVar};

// ---------------------------------------------------------------------------
// Test_TfScoped — mirrors the four sub-blocks in the C++ test
// ---------------------------------------------------------------------------

/// Plain function callback — mirrors `TfScoped<> scope(&Func)`.
#[test]
fn scoped_plain_function() {
    let x = Cell::new(false);

    assert!(!x.get(), "unexpected state before scope");
    {
        x.set(true);
        let _scope = Scoped::new(|| x.set(false));
        assert!(x.get(), "unexpected state in scope");
    }
    assert!(!x.get(), "unexpected state after scope");
}

/// Closure capturing state — mirrors `TfScoped<>` with `std::bind(&BoundFunc, &x, false)`.
#[test]
fn scoped_bound_closure() {
    let x = Cell::new(false);

    assert!(!x.get(), "unexpected state before scope");
    {
        x.set(true);
        // Equivalent to std::bind(&BoundFunc, &x, false): sets x = false on exit.
        let _scope = Scoped::new(|| x.set(false));
        assert!(x.get(), "unexpected state in scope");
    }
    assert!(!x.get(), "unexpected state after scope");
}

/// Closure with captured pointer — mirrors `TfScoped<void(*)(bool*)> scope(&ResetFunc, &x)`.
#[test]
fn scoped_function_with_arg() {
    let x = Cell::new(false);

    assert!(!x.get(), "unexpected state before scope");
    {
        x.set(true);
        // ResetFunc takes a bool* and sets *y = false; we model it as a closure.
        let _scope = Scoped::new(|| x.set(false));
        assert!(x.get(), "unexpected state in scope");
    }
    assert!(!x.get(), "unexpected state after scope");
}

/// Method pointer — mirrors `TfScoped<void(Resetter::*)()> scope(&r, &Resetter::Reset)`.
#[test]
fn scoped_method_pointer() {
    struct Resetter<'a> {
        x: &'a Cell<bool>,
    }
    impl<'a> Resetter<'a> {
        fn reset(&self) {
            self.x.set(false);
        }
    }

    let x = Cell::new(false);

    assert!(!x.get(), "unexpected state before scope");
    {
        let r = Resetter { x: &x };
        x.set(true);
        let _scope = Scoped::new(move || r.reset());
        assert!(x.get(), "unexpected state in scope");
    }
    assert!(!x.get(), "unexpected state after scope");
}

// ---------------------------------------------------------------------------
// Test_TfScopedVar — mirrors both sub-blocks
// ---------------------------------------------------------------------------

/// `TfScopedVar<bool>` — set true inside scope, restored to false after.
#[test]
fn scoped_var_bool() {
    let mut x = false;
    {
        let guard = ScopedVar::new(&mut x, true);
        assert!(*guard.get(), "bool: unexpected state in scope");
    }
    assert!(!x, "bool: unexpected state after scope");
}

/// `TfScopedAutoVar` (deduced type) for int — mirrors `TfScopedAutoVar scope(y, 8)`.
#[test]
fn scoped_var_int() {
    let mut y = 5i32;
    {
        let guard = ScopedVar::new(&mut y, 8);
        assert_eq!(*guard.get(), 8, "int: unexpected state in scope");
    }
    assert_eq!(y, 5, "int: unexpected state after scope");
}
