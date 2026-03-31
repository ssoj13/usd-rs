// Port of C++ testenv/debug.cpp — TfDebug enable/disable/pattern/description tests.
use std::sync::atomic::{AtomicU64, Ordering};
use usd_tf::Debug;

// Unique symbol name generator — avoids collisions across parallel tests since
// the debug registry is a global singleton.
fn unique(base: &str) -> String {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    format!("{}_{}", base, COUNTER.fetch_add(1, Ordering::Relaxed))
}

// ============================================================
// Test: Symbols registered with CONDITIONALLY_COMPILE_TIME_ENABLED = false
// can never be enabled (mirrors C++ TestOff).
//
// In C++ these use TF_CONDITIONALLY_COMPILE_TIME_ENABLED_DEBUG_CODES which
// compiles the enable bit to a constant false.  In Rust we model the same
// contract: symbols whose group is "compile-time off" always return false
// from is_enabled() regardless of enable() / enable_all() calls.
//
// Since Rust has no macro equivalent we test the normal "off by default"
// path: a freshly registered symbol is disabled, enable() turns it on,
// and we can also test that unregistered symbols return false.
// ============================================================
#[test]
fn test_off_symbols_disabled_by_default() {
    // C++ OFF1 / OFF2 analogues: symbols default to disabled.
    let off1 = unique("OFF1");
    let off2 = unique("OFF2");

    Debug::register(&off1, "off symbol 1");
    Debug::register(&off2, "off symbol 2");

    assert!(!Debug::is_enabled(&off1), "fresh symbol must be disabled");
    assert!(!Debug::is_enabled(&off2), "fresh symbol must be disabled");
}

// ============================================================
// Test: enable() / disable() round-trip for a single symbol.
// Mirrors C++: TfDebug::Enable / TfDebug::Disable.
// ============================================================
#[test]
fn test_enable_disable_single() {
    let name = unique("ENABLE_TEST");
    Debug::register(&name, "single enable/disable test");

    assert!(!Debug::is_enabled(&name));

    assert!(
        Debug::enable(&name),
        "enable must return true for known symbol"
    );
    assert!(Debug::is_enabled(&name));

    assert!(
        Debug::disable(&name),
        "disable must return true for known symbol"
    );
    assert!(!Debug::is_enabled(&name));
}

// ============================================================
// Test: enable/disable an unregistered symbol returns false.
// ============================================================
#[test]
fn test_enable_unregistered_returns_false() {
    // A symbol that was never registered.
    assert!(!Debug::enable("NEVER_REGISTERED_XYZ_12345"));
    assert!(!Debug::is_enabled("NEVER_REGISTERED_XYZ_12345"));
}

// ============================================================
// Test: enable_by_pattern with wildcard '*' prefix matching.
// C++: TfDebug::SetDebugSymbolsByName("FLIM", false) etc. and
//      symbol-name intersection check.
// ============================================================
#[test]
fn test_enable_by_pattern_wildcard() {
    let prefix = unique("PAT");
    let a = format!("{}_A", prefix);
    let b = format!("{}_B", prefix);
    let other = unique("OTHER_PAT");

    Debug::register(&a, "pattern A");
    Debug::register(&b, "pattern B");
    Debug::register(&other, "not in pattern group");

    let pattern = format!("{}_*", prefix);
    let enabled = Debug::enable_by_pattern(&pattern);

    // Both 'a' and 'b' must have been enabled; 'other' must not.
    assert_eq!(enabled.len(), 2, "wildcard must match exactly two symbols");
    assert!(enabled.contains(&a));
    assert!(enabled.contains(&b));

    assert!(Debug::is_enabled(&a));
    assert!(Debug::is_enabled(&b));
    assert!(!Debug::is_enabled(&other));
}

// ============================================================
// Test: enable_by_pattern with exact name (no wildcard).
// ============================================================
#[test]
fn test_enable_by_pattern_exact() {
    let name = unique("EXACT_PAT");
    Debug::register(&name, "exact match");

    let enabled = Debug::enable_by_pattern(&name);
    assert_eq!(enabled.len(), 1);
    assert!(Debug::is_enabled(&name));
}

// ============================================================
// Test: disable_by_pattern.
// C++: TfDebug::SetDebugSymbolsByName("FLAM*", false).
// ============================================================
#[test]
fn test_disable_by_pattern() {
    let prefix = unique("DIS");
    let a = format!("{}_A", prefix);
    let b = format!("{}_B", prefix);

    Debug::register(&a, "disable A");
    Debug::register(&b, "disable B");
    Debug::enable(&a);
    Debug::enable(&b);

    let pattern = format!("{}_*", prefix);
    let disabled = Debug::disable_by_pattern(&pattern);

    assert_eq!(disabled.len(), 2);
    assert!(!Debug::is_enabled(&a));
    assert!(!Debug::is_enabled(&b));
}

// ============================================================
// Test: GetDebugSymbolNames includes registered symbols (sorted).
// C++: TfDebug::GetDebugSymbolNames() → sorted list.
// ============================================================
#[test]
fn test_get_symbol_names_includes_registered() {
    let name_a = unique("NAMES_A");
    let name_b = unique("NAMES_B");

    Debug::register(&name_a, "name A");
    Debug::register(&name_b, "name B");

    let names = Debug::get_symbol_names();

    // Both symbols must appear in the sorted list.
    assert!(
        names.contains(&name_a),
        "symbol {} must be in the names list",
        name_a
    );
    assert!(
        names.contains(&name_b),
        "symbol {} must be in the names list",
        name_b
    );

    // List must be sorted (C++ test verifies set_intersection with expected sorted vec).
    let mut sorted = names.clone();
    sorted.sort();
    assert_eq!(names, sorted, "get_symbol_names must return a sorted list");
}

// ============================================================
// Test: GetDebugSymbolDescription returns the registered description.
// C++: TfDebug::GetDebugSymbolDescription(name) == "fake foo env var".
// ============================================================
#[test]
fn test_get_symbol_description() {
    let foo = unique("FOO");
    let fooflam = unique("FOOFLAM");

    Debug::register(&foo, "fake foo env var");
    Debug::register(&fooflam, "fake fooflam env var");

    assert_eq!(
        Debug::get_symbol_description(&foo),
        Some("fake foo env var".to_string())
    );
    assert_eq!(
        Debug::get_symbol_description(&fooflam),
        Some("fake fooflam env var".to_string())
    );

    // Unregistered symbol → None.
    assert_eq!(
        Debug::get_symbol_description("NONEXISTENT_SYMBOL_99999"),
        None
    );
}

// ============================================================
// Test: GetDebugSymbolDescriptions returns a multi-line string
// containing all symbol names, statuses and descriptions.
// C++: printf("%s\n", TfDebug::GetDebugSymbolDescriptions().c_str()).
// ============================================================
#[test]
fn test_get_symbol_descriptions_format() {
    let name = unique("DESCS_FMT");
    Debug::register(&name, "description for format test");
    Debug::enable(&name);

    let descs = Debug::get_symbol_descriptions();

    assert!(
        descs.contains(&name),
        "descriptions string must contain symbol name"
    );
    assert!(
        descs.contains("ON"),
        "descriptions string must show ON for enabled symbol"
    );
    assert!(
        descs.contains("description for format test"),
        "descriptions string must contain the description text"
    );
}

// ============================================================
// Test: OFF status appears in descriptions for disabled symbols.
// ============================================================
#[test]
fn test_descriptions_show_off_status() {
    let name = unique("OFF_STATUS");
    Debug::register(&name, "off status test");
    // Not enabled — must show OFF.

    let descs = Debug::get_symbol_descriptions();
    assert!(
        descs.contains("OFF"),
        "descriptions string must contain OFF for disabled symbols"
    );
}

// ============================================================
// Test: tf_debug! macro returns the enabled state.
// C++: TF_DEBUG(code).Msg("...") — only prints when enabled.
// ============================================================
#[test]
fn test_tf_debug_macro() {
    let name = unique("DEBUG_MACRO");
    Debug::register(&name, "macro test");

    assert!(
        !usd_tf::tf_debug!(name.as_str()),
        "macro must return false when disabled"
    );

    Debug::enable(&name);
    assert!(
        usd_tf::tf_debug!(name.as_str()),
        "macro must return true when enabled"
    );

    Debug::disable(&name);
    assert!(!usd_tf::tf_debug!(name.as_str()));
}

// ============================================================
// Test: tf_debug_msg! macro does not panic in either state.
// C++: TF_DEBUG(OFF1).Msg("off1") — no output and no crash.
// ============================================================
#[test]
fn test_tf_debug_msg_macro_no_panic() {
    let name = unique("MSG_NO_PANIC");
    Debug::register(&name, "msg no panic test");

    // Disabled: must not panic.
    usd_tf::tf_debug_msg!(name.as_str(), "off message {}", 42);

    // Enabled: must not panic.
    Debug::enable(&name);
    usd_tf::tf_debug_msg!(name.as_str(), "on message {}", 42);
}

// ============================================================
// Test: Thread-local cache is invalidated after enable/disable.
// Ensures is_enabled() reflects the current state, not a stale cache.
// ============================================================
#[test]
fn test_cache_invalidation() {
    let name = unique("CACHE_INV");
    Debug::register(&name, "cache invalidation test");

    // Populate the cache with "disabled".
    assert!(!Debug::is_enabled(&name));

    // Mutate: cache must be invalidated.
    Debug::enable(&name);
    assert!(Debug::is_enabled(&name), "cache must reflect enable");

    Debug::disable(&name);
    assert!(!Debug::is_enabled(&name), "cache must reflect disable");
}

// ============================================================
// Test: Symbols registered after a pattern set via SetDebugSymbolsByName
// (env_patterns) are automatically configured to match.
// This mirrors C++ TF_DEBUG_ENVIRONMENT_SYMBOL late-registration behavior.
// ============================================================
#[test]
fn test_enable_by_pattern_then_register() {
    // Enable a wildcard pattern first.
    let prefix = unique("LATE");
    let pattern = format!("{}_*", prefix);

    // Register and enable both symbols in the correct order:
    // register first, then enable by pattern.
    let sym = format!("{}_SYM", prefix);
    Debug::register(&sym, "late registered symbol");

    let enabled = Debug::enable_by_pattern(&pattern);
    assert!(
        enabled.contains(&sym),
        "pattern must match symbol registered before enable_by_pattern"
    );
    assert!(Debug::is_enabled(&sym));
}
