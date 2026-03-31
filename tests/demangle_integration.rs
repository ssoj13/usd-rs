//! Integration test for symbol demangling.

use std::collections::HashMap;
use usd::arch::{arch_demangle, arch_get_demangled_type_name, arch_get_pretty_type_name};

#[test]
fn test_basic_type_names() {
    let name = arch_get_demangled_type_name::<Vec<String>>();
    assert!(!name.is_empty());
    assert!(name.contains("Vec") || name.contains("vec"));

    let pretty = arch_get_pretty_type_name::<Vec<String>>();
    assert!(!pretty.contains("alloc::"));
    assert!(!pretty.contains("std::"));
}

#[test]
fn test_complex_types() {
    let name = arch_get_pretty_type_name::<HashMap<String, Vec<i32>>>();
    assert!(!name.is_empty());
    assert!(name.contains("HashMap") || name.contains("map"));
}

#[test]
fn test_option_result() {
    let opt_name = arch_get_pretty_type_name::<Option<i32>>();
    assert!(opt_name.contains("Option") || opt_name.contains("option"));

    let res_name = arch_get_pretty_type_name::<Result<String, ()>>();
    assert!(res_name.contains("Result") || res_name.contains("result"));
}

#[test]
fn test_demangle_returns_none_for_invalid() {
    let result = arch_demangle("not_a_valid_mangled_symbol_12345");
    assert!(result.is_none());
}

#[test]
fn test_nested_generic_types() {
    let name = arch_get_pretty_type_name::<Option<Vec<HashMap<String, i32>>>>();
    assert!(!name.contains("alloc::"));
    assert!(!name.contains("std::"));
}
