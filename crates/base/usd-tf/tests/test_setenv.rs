// Port of testenv/setenv.cpp to Rust integration tests.
//
// The C++ test has two non-Python sub-tests:
//   _TestSetenvNoInit  — set/unset without Python initialised
//   _TestSetenvInit    — set/unset after Python initialised (Python block skipped)
//
// The Python-dependent paths (TfPySetenv / TfPyUnsetenv) have no Rust
// equivalent and are intentionally omitted.
//
// Unique var names are used per test to avoid inter-test interference.

use usd_tf::getenv::tf_getenv;
use usd_tf::setenv::{tf_setenv, tf_unsetenv};

// ---------------------------------------------------------------------------
// _TestSetenvNoInit equivalent
// ---------------------------------------------------------------------------

#[test]
fn test_setenv_no_init_set_and_unset() {
    let env_name = "TF_INTTEST_SETENV_NO_INIT";
    let env_val = "TestSetenvNoInit";

    // TfSetenv must succeed and make the value visible via TfGetenv.
    assert!(tf_setenv(env_name, env_val));
    assert_eq!(tf_getenv(env_name, ""), env_val);

    // TfUnsetenv must succeed and remove the value from the environment.
    assert!(tf_unsetenv(env_name));
    assert_eq!(tf_getenv(env_name, ""), "");
}

// ---------------------------------------------------------------------------
// _TestSetenvInit equivalent (Python parts omitted — no Python in Rust)
// ---------------------------------------------------------------------------

#[test]
fn test_setenv_init_set_and_unset() {
    let env_name = "TF_INTTEST_SETENV_INIT";
    let env_val = "TestSetenvInit";

    tf_setenv(env_name, env_val);
    assert_eq!(tf_getenv(env_name, ""), env_val);

    tf_unsetenv(env_name);
    assert_eq!(tf_getenv(env_name, ""), "");
}

// ---------------------------------------------------------------------------
// Additional coverage: overwriting an existing value
// ---------------------------------------------------------------------------

#[test]
fn test_setenv_overwrite() {
    let env_name = "TF_INTTEST_SETENV_OVERWRITE";

    assert!(tf_setenv(env_name, "first"));
    assert_eq!(tf_getenv(env_name, ""), "first");

    assert!(tf_setenv(env_name, "second"));
    assert_eq!(tf_getenv(env_name, ""), "second");

    tf_unsetenv(env_name);
}

// ---------------------------------------------------------------------------
// Invalid names must be rejected
// ---------------------------------------------------------------------------

#[test]
fn test_setenv_invalid_empty_name() {
    assert!(!tf_setenv("", "value"));
}

#[test]
fn test_setenv_invalid_name_with_equals() {
    assert!(!tf_setenv("NAME=VALUE", "value"));
}

#[test]
fn test_unsetenv_invalid_empty_name() {
    assert!(!tf_unsetenv(""));
}

#[test]
fn test_unsetenv_invalid_name_with_equals() {
    assert!(!tf_unsetenv("NAME=VALUE"));
}
