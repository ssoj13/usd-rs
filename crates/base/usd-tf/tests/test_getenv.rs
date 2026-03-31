// Port of testenv/getenv.cpp to Rust integration tests.
// Each test uses a unique env var name to avoid parallel test race conditions.

use usd_tf::getenv::{tf_getenv, tf_getenv_bool, tf_getenv_int};

#[test]
fn test_tf_getenv_set_and_removed() {
    let var = "TF_TEST_GE_STR1";
    #[allow(unsafe_code)]
    unsafe {
        std::env::set_var(var, "testing");
    }
    assert_eq!(tf_getenv(var, "bogusValue"), "testing");

    #[allow(unsafe_code)]
    unsafe {
        std::env::remove_var(var);
    }
    assert_eq!(tf_getenv(var, "bogusValue"), "bogusValue");
}

#[test]
fn test_tf_getenv_int_set_and_removed() {
    let var = "TF_TEST_GE_INT1";
    #[allow(unsafe_code)]
    unsafe {
        std::env::set_var(var, "42");
    }
    assert_eq!(tf_getenv_int(var, 99), 42);

    #[allow(unsafe_code)]
    unsafe {
        std::env::remove_var(var);
    }
    assert_eq!(tf_getenv_int(var, 99), 99);
}

#[test]
fn test_tf_getenv_bool_true_lowercase() {
    let var = "TF_TEST_GE_BT1";
    #[allow(unsafe_code)]
    unsafe {
        std::env::set_var(var, "true");
    }
    assert!(tf_getenv_bool(var, false));
}

#[test]
fn test_tf_getenv_bool_true_uppercase() {
    let var = "TF_TEST_GE_BT2";
    #[allow(unsafe_code)]
    unsafe {
        std::env::set_var(var, "TRUE");
    }
    assert!(tf_getenv_bool(var, false));
}

#[test]
fn test_tf_getenv_bool_yes_lowercase() {
    let var = "TF_TEST_GE_BY1";
    #[allow(unsafe_code)]
    unsafe {
        std::env::set_var(var, "yes");
    }
    assert!(tf_getenv_bool(var, false));
}

#[test]
fn test_tf_getenv_bool_yes_uppercase() {
    let var = "TF_TEST_GE_BY2";
    #[allow(unsafe_code)]
    unsafe {
        std::env::set_var(var, "YES");
    }
    assert!(tf_getenv_bool(var, false));
}

#[test]
fn test_tf_getenv_bool_one() {
    let var = "TF_TEST_GE_B1";
    #[allow(unsafe_code)]
    unsafe {
        std::env::set_var(var, "1");
    }
    assert!(tf_getenv_bool(var, false));
}

#[test]
fn test_tf_getenv_bool_on_uppercase() {
    let var = "TF_TEST_GE_BON1";
    #[allow(unsafe_code)]
    unsafe {
        std::env::set_var(var, "ON");
    }
    assert!(tf_getenv_bool(var, false));
}

#[test]
fn test_tf_getenv_bool_on_lowercase() {
    let var = "TF_TEST_GE_BON2";
    #[allow(unsafe_code)]
    unsafe {
        std::env::set_var(var, "on");
    }
    assert!(tf_getenv_bool(var, false));
}

#[test]
fn test_tf_getenv_bool_false_with_false_default() {
    let var = "TF_TEST_GE_BF1";
    #[allow(unsafe_code)]
    unsafe {
        std::env::set_var(var, "false");
    }
    assert!(!tf_getenv_bool(var, false));
}

#[test]
fn test_tf_getenv_bool_false_with_true_default() {
    let var = "TF_TEST_GE_BF2";
    #[allow(unsafe_code)]
    unsafe {
        std::env::set_var(var, "false");
    }
    assert!(!tf_getenv_bool(var, true));
}

#[test]
fn test_tf_getenv_bool_garbage_with_false_default() {
    let var = "TF_TEST_GE_BG1";
    #[allow(unsafe_code)]
    unsafe {
        std::env::set_var(var, "someothercrap");
    }
    assert!(!tf_getenv_bool(var, false));
}

#[test]
fn test_tf_getenv_bool_garbage_with_true_default() {
    let var = "TF_TEST_GE_BG2";
    #[allow(unsafe_code)]
    unsafe {
        std::env::set_var(var, "someothercrap");
    }
    assert!(!tf_getenv_bool(var, true));
}
