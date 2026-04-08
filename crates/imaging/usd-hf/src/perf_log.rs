//! HF performance logging and memory tagging macros.
//!
//! In the C++ version, these macros inject malloc tags for memory profiling.
//! In Rust, these are no-ops or compile to empty code, as Rust has different
//! memory profiling mechanisms (valgrind, heaptrack, cargo-instruments, etc.)

/// No-op macro for function-level malloc tagging.
///
/// In C++, this creates an auto-mallocTag with the function name.
/// In Rust, this is a no-op.
#[macro_export]
macro_rules! hf_malloc_tag_function {
    () => {
        // No-op in Rust
    };
}

/// No-op macro for named malloc tagging.
///
/// In C++, this creates an auto-mallocTag with the given tag.
/// In Rust, this is a no-op.
///
/// # Example
///
/// ```ignore
/// use usd_hf::hf_malloc_tag;
///
/// fn process_data() {
///     hf_malloc_tag!("data_processing");
///     // ... allocate memory ...
/// }
/// ```
#[macro_export]
macro_rules! hf_malloc_tag {
    ($tag:expr) => {
        // No-op in Rust
    };
}

/// No-op macro for trace function scope.
///
/// In C++, this uses the trace library for function profiling.
/// In Rust, use `tracing` crate or similar for actual profiling.
///
/// # Example
///
/// ```ignore
/// use usd_hf::hf_trace_function_scope;
///
/// fn expensive_operation() {
///     hf_trace_function_scope!("expensive_op");
///     // ... do work ...
/// }
/// ```
#[macro_export]
macro_rules! hf_trace_function_scope {
    ($tag:expr) => {
        // No-op in Rust - use tracing crate for actual profiling
    };
}

// Re-export macros at module level
pub use hf_malloc_tag;
pub use hf_malloc_tag_function;
pub use hf_trace_function_scope;

#[cfg(test)]
mod tests {
    #[test]
    fn test_perf_macros_compile() {
        // These macros should compile without errors
        hf_malloc_tag_function!();
        hf_malloc_tag!("test_tag");
        hf_trace_function_scope!("test_scope");
    }

    #[test]
    fn test_perf_macros_in_function() {
        fn tagged_function() -> i32 {
            hf_malloc_tag_function!();
            hf_malloc_tag!("allocation");
            42
        }

        assert_eq!(tagged_function(), 42);
    }
}
