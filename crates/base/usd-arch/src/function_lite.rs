//! Function name macros for architecture-independent function name access.
//!
//! This module provides macros for getting the current function name and related
//! information in an architecture-independent manner. This is the Rust equivalent
//! of `pxr/base/arch/functionLite.h`.
//!
//! # Example
//!
//! ```ignore
//! use usd_arch::arch_function;
//!
//! fn my_function() {
//!     let func_name = arch_function!();
//!     println!("Current function: {}", func_name);
//! }
//! ```

/// Macro to get the current function name.
///
/// This is equivalent to C++ `__ARCH_FUNCTION__` which expands to `__func__`.
/// In Rust, this uses `std::any::type_name` to get function information.
///
/// # Example
///
/// ```ignore
/// use usd_arch::arch_function;
///
/// fn test() {
///     let name = arch_function!();
///     assert!(name.contains("test"));
/// }
/// ```
#[macro_export]
macro_rules! arch_function {
    () => {{
        fn f() {}
        fn type_name_of<T>(_: T) -> &'static str {
            std::any::type_name::<T>()
        }
        let name = type_name_of(f);
        // Extract function name from the type name
        // Format is typically: "crate::module::function::f"
        // We want to extract "function"
        if let Some(pos) = name.rfind("::") {
            if let Some(prev_pos) = name[..pos].rfind("::") {
                &name[prev_pos + 2..pos]
            } else {
                &name[..pos]
            }
        } else {
            name
        }
    }};
}

/// Macro to get the "pretty" function name with full signature.
///
/// This is equivalent to C++ `__ARCH_PRETTY_FUNCTION__` which expands to
/// `__PRETTY_FUNCTION__` on GCC/Clang or `__FUNCSIG__` on MSVC.
/// In Rust, this uses `std::any::type_name` to get the full function signature.
///
/// # Example
///
/// ```ignore
/// use usd_arch::arch_pretty_function;
///
/// fn test_function() {
///     let pretty = arch_pretty_function!();
///     println!("Pretty function: {}", pretty);
/// }
/// ```
#[macro_export]
macro_rules! arch_pretty_function {
    () => {{
        fn f() {}
        fn type_name_of<T>(_: T) -> &'static str {
            std::any::type_name::<T>()
        }
        let name = type_name_of(f);
        // Return the full type name which includes the function signature
        if let Some(pos) = name.rfind("::f") {
            &name[..pos]
        } else {
            name
        }
    }};
}

/// Macro to get the current file name.
///
/// This is equivalent to C++ `__ARCH_FILE__` which expands to `__FILE__`.
/// In Rust, this uses the built-in `file!()` macro.
///
/// # Example
///
/// ```ignore
/// use usd_arch::arch_file;
///
/// let file = arch_file!();
/// println!("Current file: {}", file);
/// ```
#[macro_export]
macro_rules! arch_file {
    () => {
        file!()
    };
}

#[cfg(test)]
mod tests {
    #[allow(unused_imports)]
    use super::*;

    #[test]
    fn test_arch_function() {
        fn test_func() -> &'static str {
            arch_function!()
        }

        let name = test_func();
        assert!(
            name.contains("test_func"),
            "Function name should contain 'test_func', got: {}",
            name
        );
    }

    #[test]
    fn test_arch_pretty_function() {
        fn test_pretty_func() -> &'static str {
            arch_pretty_function!()
        }

        let pretty = test_pretty_func();
        assert!(
            pretty.contains("test_pretty_func"),
            "Pretty function should contain 'test_pretty_func', got: {}",
            pretty
        );
    }

    #[test]
    fn test_arch_file() {
        let file = arch_file!();
        assert!(
            file.contains("function_lite.rs"),
            "File should contain 'function_lite.rs', got: {}",
            file
        );
    }
}
