//! Example demonstrating debugger interaction utilities.

use usd::arch::{arch_debugger_is_attached, arch_debugger_wait};

fn main() {
    println!("Debugger Detection Example");
    println!("==========================\n");

    // Check if debugger is attached
    if arch_debugger_is_attached() {
        println!("✓ Debugger is currently attached");
    } else {
        println!("✗ No debugger detected");
    }

    // Demonstrate wait flag
    println!("\nSetting debugger wait flag...");
    arch_debugger_wait(true);
    println!("✓ Wait flag set");

    arch_debugger_wait(false);
    println!("✓ Wait flag cleared");

    println!("\nNote: Call arch_debugger_trap() to trigger a breakpoint");
    println!("Note: Call arch_abort(true) to abort with debugger notification");
}
