//! Stack trace demonstration example.

use usd::arch::{
    arch_get_stack_trace, arch_get_stack_trace_string, arch_log_stack_trace,
    arch_print_stack_trace_stderr, arch_set_stack_trace_callback,
};

fn level_3() {
    println!("\n=== Printing stack trace to stderr ===");
    arch_print_stack_trace_stderr();
}

fn level_2() {
    level_3();
}

fn level_1() {
    level_2();
}

fn main() {
    println!("Stack Trace Demo\n");

    // Example 1: Get stack frames programmatically
    println!("=== Getting stack frames programmatically ===");
    let frames = arch_get_stack_trace(10);
    println!("Captured {} frames:", frames.len());
    for (idx, frame) in frames.iter().enumerate() {
        println!("  Frame {}: {}", idx, frame);
    }

    // Example 2: Get stack trace as string
    println!("\n=== Getting stack trace as string ===");
    let trace_str = arch_get_stack_trace_string(5);
    println!("{}", trace_str);

    // Example 3: Print to stderr through nested calls
    level_1();

    // Example 4: Log with reason
    println!("\n=== Logging with reason ===");
    arch_log_stack_trace("Demonstration of logging functionality");

    // Example 5: Custom callback
    println!("\n=== Using custom callback ===");
    arch_set_stack_trace_callback(|skip| {
        let trace = format!("CUSTOM LOGGER: skip={}", skip);
        println!("{}", trace);
        trace
    });
    arch_log_stack_trace("Custom callback test");

    println!("\n=== Demo complete ===");
}
