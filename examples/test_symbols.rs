//! Test program for arch::symbols module.
//!
//! Demonstrates symbol lookup and address information retrieval.

use usd::arch::{arch_get_address_info, arch_get_current_function_address};

fn example_function() {
    println!("=== Example Function ===");

    // Get info about this function itself
    let addr = example_function as *const () as usize;
    println!("Function address: {:#x}", addr);

    if let Some(info) = arch_get_address_info(addr) {
        println!("Module: {}", info.file_name.display());
        println!("Symbol: {}", info.symbol_name);
        println!("Base address: {:#x}", info.base_address);
        println!("Offset from symbol: {:#x}", info.offset_from_symbol);
    } else {
        println!("Failed to get address info");
    }
}

fn nested_caller() {
    println!("\n=== Nested Caller ===");
    nested_callee();
}

fn nested_callee() {
    // Get return address (address in caller)
    let return_addr = arch_get_current_function_address();
    println!("Return address: {:#x}", return_addr);

    if let Some(info) = arch_get_address_info(return_addr) {
        println!("Caller module: {}", info.file_name.display());
        println!("Caller symbol: {}", info.symbol_name);
    }
}

fn test_null_address() {
    println!("\n=== Null Address Test ===");
    let info = arch_get_address_info(0);
    assert!(info.is_none(), "Null address should return None");
    println!("Null address correctly returns None");
}

fn test_main_function() {
    println!("\n=== Main Function ===");
    let addr = main as *const () as usize;

    if let Some(info) = arch_get_address_info(addr) {
        println!("Main function:");
        println!("  Address: {:#x}", addr);
        println!("  Module: {}", info.file_name.display());
        println!("  Symbol: {}", info.symbol_name);
        println!("  Base: {:#x}", info.base_address);

        // Verify base address is less than function address
        assert!(info.base_address <= addr);
        println!("  ✓ Base address validation passed");
    }
}

fn main() {
    println!("Symbol Lookup Test\n");
    println!("Platform: {}", std::env::consts::OS);
    println!("Architecture: {}\n", std::env::consts::ARCH);

    example_function();
    nested_caller();
    test_null_address();
    test_main_function();

    println!("\n=== All Tests Passed ===");
}
