//! Example demonstrating symbol demangling functionality.
//!
//! Run with: cargo run --example demangle_example --features cpp-demangle

use std::collections::HashMap;
use usd::arch::{
    arch_demangle, arch_demangle_function_name, arch_get_demangled_type_name,
    arch_get_pretty_type_name,
};

fn main() {
    println!("=== Symbol Demangling Example ===\n");

    // Example 1: Get type names
    println!("1. Type Names:");
    println!(
        "   Vec<String> (demangled): {}",
        arch_get_demangled_type_name::<Vec<String>>()
    );
    println!(
        "   Vec<String> (pretty):    {}",
        arch_get_pretty_type_name::<Vec<String>>()
    );
    println!();

    // Example 2: Complex types
    println!("2. Complex Types:");
    println!(
        "   HashMap<String, Vec<i32>> (demangled): {}",
        arch_get_demangled_type_name::<HashMap<String, Vec<i32>>>()
    );
    println!(
        "   HashMap<String, Vec<i32>> (pretty):    {}",
        arch_get_pretty_type_name::<HashMap<String, Vec<i32>>>()
    );
    println!();

    // Example 3: Option and Result types
    println!("3. Standard Library Types:");
    println!(
        "   Option<i32> (pretty): {}",
        arch_get_pretty_type_name::<Option<i32>>()
    );
    println!(
        "   Result<String, std::io::Error> (pretty): {}",
        arch_get_pretty_type_name::<Result<String, std::io::Error>>()
    );
    println!();

    // Example 4: Manual symbol demangling
    println!("4. Manual Symbol Demangling:");

    // Try some Rust symbols
    let rust_symbols = vec!["_RNvC6my_lib8my_function", "_ZN3foo3barE"];

    for symbol in rust_symbols {
        match arch_demangle(symbol) {
            Some(demangled) => println!("   {} -> {}", symbol, demangled),
            None => println!("   {} (could not demangle)", symbol),
        }
    }
    println!();

    // Example 5: Function name demangling
    println!("5. Function Name Demangling:");
    let function_names = vec!["_Z3foov", "regular_function", "_Z9my_funcPKc"];

    for fname in function_names {
        let demangled = arch_demangle_function_name(fname);
        if demangled != fname {
            println!("   {} -> {}", fname, demangled);
        } else {
            println!("   {} (no change)", fname);
        }
    }
    println!();

    println!("=== Example Complete ===");
}
