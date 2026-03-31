// Port of testenv/envSetting.cpp to Rust integration tests.
//
// The C++ test exercises TfGetEnvSetting via global statics that are
// pre-loaded from the process environment before the test body runs.
// Because Rust statics with OnceLock cache on first access we must set
// the environment variables **before** the first call to `.get()`.
// Each test uses a unique var name so tests are independent even under
// parallel execution.

use usd_tf::env_setting::{EnvSetting, StringEnvSetting};

// ---------------------------------------------------------------------------
// Bool setting – unset (stays at default false)
// ---------------------------------------------------------------------------
static BOOL_SETTING_NOT_SET: EnvSetting<bool> = EnvSetting::new(
    "TF_INTTEST_BOOL_ENV_SETTING_X",
    false,
    "bool env setting (not set by test)",
);

#[test]
fn test_bool_env_setting_not_set_returns_default() {
    // Env var deliberately not set, expect compile-time default.
    assert!(!BOOL_SETTING_NOT_SET.get());
}

// ---------------------------------------------------------------------------
// Bool setting – set to "1" (truthy)
// ---------------------------------------------------------------------------
static BOOL_SETTING_SET: EnvSetting<bool> =
    EnvSetting::new("TF_INTTEST_BOOL_ENV_SETTING", false, "bool env setting");

#[test]
fn test_bool_env_setting_set_overrides_default() {
    // SAFETY: single-threaded access; static is initialised exactly once so
    // the env var must be present before the first .get() call.
    #[allow(unsafe_code)]
    unsafe {
        std::env::set_var("TF_INTTEST_BOOL_ENV_SETTING", "1");
    }
    assert!(BOOL_SETTING_SET.get());
}

// ---------------------------------------------------------------------------
// Int setting – unset (stays at default 1)
// ---------------------------------------------------------------------------
static INT_SETTING_NOT_SET: EnvSetting<i32> = EnvSetting::new(
    "TF_INTTEST_INT_ENV_SETTING_X",
    1,
    "int env setting (not set by test)",
);

#[test]
fn test_int_env_setting_not_set_returns_default() {
    assert_eq!(INT_SETTING_NOT_SET.get(), 1);
}

// ---------------------------------------------------------------------------
// Int setting – set to "123"
// ---------------------------------------------------------------------------
static INT_SETTING_SET: EnvSetting<i32> =
    EnvSetting::new("TF_INTTEST_INT_ENV_SETTING", 1, "int env setting");

#[test]
fn test_int_env_setting_set_overrides_default() {
    #[allow(unsafe_code)]
    unsafe {
        std::env::set_var("TF_INTTEST_INT_ENV_SETTING", "123");
    }
    assert_eq!(INT_SETTING_SET.get(), 123);
}

// ---------------------------------------------------------------------------
// String setting – unset (stays at default "default")
// ---------------------------------------------------------------------------
static STRING_SETTING_NOT_SET: StringEnvSetting = StringEnvSetting::new(
    "TF_INTTEST_STRING_ENV_SETTING_X",
    "default",
    "string env setting (not set by test)",
);

#[test]
fn test_string_env_setting_not_set_returns_default() {
    assert_eq!(STRING_SETTING_NOT_SET.get(), "default");
}

// ---------------------------------------------------------------------------
// String setting – set to "alpha"
// ---------------------------------------------------------------------------
static STRING_SETTING_SET: StringEnvSetting = StringEnvSetting::new(
    "TF_INTTEST_STRING_ENV_SETTING",
    "default",
    "string env setting",
);

#[test]
fn test_string_env_setting_set_overrides_default() {
    #[allow(unsafe_code)]
    unsafe {
        std::env::set_var("TF_INTTEST_STRING_ENV_SETTING", "alpha");
    }
    assert_eq!(STRING_SETTING_SET.get(), "alpha");
}

// ---------------------------------------------------------------------------
// Post-registry-manager equivalent:
// C++ calls TfGetEnvSetting early (via ARCH_CONSTRUCTOR) to verify that
// accessing a setting during static init doesn't trigger a double-define.
// In Rust the equivalent is accessing a static EnvSetting from another
// static's initialiser, which OnceLock handles safely.
// ---------------------------------------------------------------------------
static POST_SETTING_NOT_SET: EnvSetting<bool> = EnvSetting::new(
    "TF_INTTEST_POST_ENV_SETTING_X",
    false,
    "post-registry-manager setting (not set by test)",
);

#[test]
fn test_post_registry_setting_not_set_returns_default() {
    // Mirrors: TF_AXIOM(TfGetEnvSetting(TF_TEST_POST_ENV_SETTING_X) == false)
    assert!(!POST_SETTING_NOT_SET.get());
}

// ---------------------------------------------------------------------------
// Accessing a setting before the env var is set should return the default,
// and the cached value must not be invalidated by a later set_var call.
// This mirrors the C++ OnceLock / std::call_once semantics.
// ---------------------------------------------------------------------------
static CACHED_SETTING: EnvSetting<i32> =
    EnvSetting::new("TF_INTTEST_CACHED_SETTING", 42, "cached value test");

#[test]
fn test_setting_value_is_cached_after_first_access() {
    // First access — env var absent, caches 42.
    assert_eq!(CACHED_SETTING.get(), 42);

    // Change env var — must NOT affect the already-cached value.
    #[allow(unsafe_code)]
    unsafe {
        std::env::set_var("TF_INTTEST_CACHED_SETTING", "999");
    }
    assert_eq!(CACHED_SETTING.get(), 42);

    #[allow(unsafe_code)]
    unsafe {
        std::env::remove_var("TF_INTTEST_CACHED_SETTING");
    }
}
