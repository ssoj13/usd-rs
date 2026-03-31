//! Compiler pragma utilities.
//!
//! This module provides Rust equivalents of compiler pragmas used in the C++
//! codebase. Since Rust doesn't have pragmas like C++, this module provides
//! documentation and guidance for achieving similar effects using Rust attributes.
//!
//! This is the Rust equivalent of `pxr/base/arch/pragmas.h`.
//!
//! # Example
//!
//! ```rust
//! // In Rust, use attributes instead of pragmas:
//! #[allow(dead_code)]
//! fn unused_function() {}
//! ```

// Note: In Rust, compiler pragmas are handled via attributes rather than
// preprocessor directives. This module exists primarily for documentation
// and API compatibility. The actual pragma functionality is achieved through
// Rust attributes like #[allow(...)], #[warn(...)], etc.

/// Documentation module for Rust equivalents of C++ pragmas.
///
/// In Rust, compiler warnings and errors are controlled via attributes:
/// - `#[allow(warning_name)]` - Suppress a warning
/// - `#[warn(warning_name)]` - Enable a warning
/// - `#[deny(warning_name)]` - Treat warning as error
/// - `#[forbid(warning_name)]` - Same as deny but cannot be overridden
pub mod push_pop {
    //! Rust equivalent of C++ pragma push/pop.
    //!
    //! In Rust, use attribute scopes instead:
    //!
    //! ```rust
    //! // Suppress warning for this block
    //! #[allow(dead_code)]
    //! {
    //!     let unused = 42;
    //! }
    //! ```
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_pragmas_module() {
        // This module exists for documentation purposes
        // Rust pragmas are handled via attributes, not preprocessor directives
        assert!(true);
    }
}
