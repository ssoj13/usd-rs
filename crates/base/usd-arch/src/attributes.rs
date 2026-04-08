//! Function and type attributes for compiler-specific optimizations.
//!
//! This module provides Rust equivalents and documentation for C++ compiler attributes
//! used in OpenUSD. Many C++ attributes map directly to Rust attributes, while others
//! are not applicable or are handled differently in Rust's type system.
//!
//! # Overview
//!
//! - **Inlining**: [`inline_never!`], [`inline_always!`] - Control function inlining
//! - **Unused markers**: [`allow_unused!`] - Suppress unused warnings
//! - **Dead code elimination**: [`used_fn!`] - Prevent linker removal
//! - **Static init/deinit**: Use `ctor` crate for constructor/destructor patterns
//! - **Visibility**: Document symbol visibility patterns
//!
//! # Rust vs C++ Attributes
//!
//! | C++ Attribute | Rust Equivalent | Notes |
//! |--------------|-----------------|-------|
//! | `ARCH_PRINTF_FUNCTION` | N/A | Rust's formatting is type-safe at compile time |
//! | `ARCH_SCANF_FUNCTION` | N/A | Use `std::io` or parsing crates |
//! | `ARCH_NOINLINE` | `#[inline(never)]` | See [`inline_never!`] |
//! | `ARCH_ALWAYS_INLINE` | `#[inline(always)]` | See [`inline_always!`] |
//! | `ARCH_UNUSED_ARG` | `#[allow(unused_variables)]` | Prefix with `_` preferred |
//! | `ARCH_UNUSED_FUNCTION` | `#[allow(dead_code)]` | See [`allow_unused!`] |
//! | `ARCH_USED_FUNCTION` | `#[used]` | See [`used_fn!`] |
//! | `ARCH_CONSTRUCTOR` | `ctor::ctor` | Use `ctor` crate |
//! | `ARCH_DESTRUCTOR` | `ctor::dtor` | Use `ctor` crate |
//! | `ARCH_EMPTY_BASES` | N/A | Rust has zero-sized type optimization |
//! | `ARCH_EXPORT/IMPORT` | `#[no_mangle]` + `pub extern "C"` | For FFI |
//! | `ARCH_HIDDEN` | `pub(crate)` or module privacy | Rust visibility |
//! | `ARCH_FALLTHROUGH` | N/A | Rust match arms don't need fallthrough |
//!
//! # Examples
//!
//! ```
//! use usd_arch::attributes::{inline_never, inline_always, allow_unused};
//!
//! // Never inline this function (useful for debugging or profiling)
//! #[inline(never)]
//! fn debug_fn() {
//!     println!("This won't be inlined");
//! }
//!
//! // Always inline this hot path
//! #[inline(always)]
//! fn hot_path() -> i32 {
//!     42
//! }
//!
//! // Allow unused function (e.g., platform-specific code)
//! #[allow(dead_code)]
//! fn platform_specific() {
//!     // Only used on certain platforms
//! }
//! ```

// ============================================================================
// Printf/Scanf-style Functions
// ============================================================================

/// Documents printf-style function formatting in C++.
///
/// **Not applicable in Rust.** Rust's `format!()`, `println!()`, and related macros
/// use compile-time type checking, making format string validation automatic.
///
/// # C++ Usage
///
/// ```cpp
/// void log_msg(int level, const char* fmt, ...) ARCH_PRINTF_FUNCTION(2, 3);
/// ```
///
/// # Rust Equivalent
///
/// ```
/// // Rust's format macros are type-safe by design
/// fn log_msg(level: i32, args: std::fmt::Arguments<'_>) {
///     println!("[{}] {}", level, args);
/// }
///
/// // Usage
/// log_msg(1, format_args!("Value: {}", 42));
/// ```
///
/// Or use macros for variadic-like behavior:
///
/// ```
/// macro_rules! log_msg {
///     ($level:expr, $($arg:tt)*) => {
///         println!("[{}] {}", $level, format!($($arg)*))
///     };
/// }
///
/// log_msg!(1, "Value: {}", 42);
/// ```
pub const PRINTF_FUNCTION_NOTE: &str =
    "Rust format strings are type-checked at compile time. Use format!() or format_args!().";

/// Documents scanf-style function parsing in C++.
///
/// **Not applicable in Rust.** Use `std::io` for reading, or parsing crates like
/// `nom`, `pest`, or `scanf` for scanf-like functionality.
///
/// # C++ Usage
///
/// ```cpp
/// int scan_vals(const char* input, const char* fmt, ...) ARCH_SCANF_FUNCTION(2, 3);
/// ```
///
/// # Rust Equivalent
///
/// ```
/// use std::io::{self, BufRead};
///
/// fn scan_vals() -> io::Result<(i32, String)> {
///     let stdin = io::stdin();
///     let line = stdin.lock().lines().next().unwrap()?;
///     let mut parts = line.split_whitespace();
///     let num: i32 = parts.next().unwrap().parse().unwrap();
///     let text = parts.next().unwrap().to_string();
///     Ok((num, text))
/// }
/// ```
///
/// Or use the `scanf` crate for format-string style parsing:
///
/// ```ignore
/// use scanf::sscanf;
/// let mut x = 0;
/// let mut s = String::new();
/// sscanf!("42 hello", "{} {}", x, s);
/// ```
pub const SCANF_FUNCTION_NOTE: &str =
    "Use std::io for input, or parsing crates like nom, pest, or scanf.";

// ============================================================================
// Inlining Control
// ============================================================================

/// Prevents function inlining.
///
/// Use `#[inline(never)]` to prevent the compiler from inlining a function.
/// This is useful for:
/// - Debugging (to ensure function appears in stack traces)
/// - Profiling (to see function in performance profiles)
/// - Reducing code bloat from generic functions
/// - Binary size optimization
///
/// # Examples
///
/// ```
/// #[inline(never)]
/// fn expensive_debug_fn(data: &[u8]) {
///     // Complex debug logic that shouldn't be inlined
///     for byte in data {
///         println!("{:02x}", byte);
///     }
/// }
/// ```
///
/// Using the macro helper:
///
/// ```
/// use usd_arch::attributes::inline_never;
///
/// inline_never! {
///     fn debug_dump(value: i32) {
///         eprintln!("Debug: {}", value);
///     }
/// }
/// ```
#[macro_export]
macro_rules! inline_never {
    (
        $(#[$meta:meta])*
        $vis:vis fn $name:ident $(<$($gen:tt),*>)? ($($args:tt)*) $(-> $ret:ty)? $body:block
    ) => {
        $(#[$meta])*
        #[inline(never)]
        $vis fn $name $(<$($gen),*>)? ($($args)*) $(-> $ret)? $body
    };
}

/// Forces function inlining.
///
/// Use `#[inline(always)]` to force the compiler to inline a function.
/// This is useful for:
/// - Hot paths where inlining is critical
/// - Zero-cost abstractions
/// - Small wrapper functions
///
/// **Warning:** Overuse can increase binary size and hurt performance due to
/// instruction cache pressure. Profile before using extensively.
///
/// # Examples
///
/// ```
/// #[inline(always)]
/// fn fast_add(a: i32, b: i32) -> i32 {
///     a + b
/// }
/// ```
///
/// Using the macro helper:
///
/// ```
/// use usd_arch::attributes::inline_always;
///
/// inline_always! {
///     fn hot_loop_kernel(x: f32, y: f32) -> f32 {
///         x * x + y * y
///     }
/// }
/// ```
#[macro_export]
macro_rules! inline_always {
    (
        $(#[$meta:meta])*
        $vis:vis fn $name:ident $(<$($gen:tt),*>)? ($($args:tt)*) $(-> $ret:ty)? $body:block
    ) => {
        $(#[$meta])*
        #[inline(always)]
        $vis fn $name $(<$($gen),*>)? ($($args)*) $(-> $ret)? $body
    };
}

// Re-export for convenience
pub use inline_always;
pub use inline_never;

// ============================================================================
// Unused Markers
// ============================================================================

/// Suppresses unused warnings for functions.
///
/// In Rust, prefer these approaches:
/// 1. Prefix with underscore: `_unused_param`
/// 2. Use `#[allow(dead_code)]` for functions
/// 3. Use `#[allow(unused_variables)]` for parameters
/// 4. Use `let _ = value;` to explicitly ignore
///
/// # Examples
///
/// ```
/// // Unused parameter (preferred style)
/// fn callback(_event: &str, data: i32) {
///     println!("Data: {}", data);
/// }
///
/// // Platform-specific function
/// #[allow(dead_code)]
/// fn windows_only() {
///     #[cfg(windows)]
///     println!("Windows!");
/// }
/// ```
///
/// Using the macro helper:
///
/// ```
/// use usd_arch::attributes::allow_unused;
///
/// allow_unused! {
///     fn experimental_feature() {
///         // Work in progress, not called yet
///     }
/// }
/// ```
#[macro_export]
macro_rules! allow_unused {
    (
        $(#[$meta:meta])*
        $vis:vis fn $name:ident $(<$($gen:tt),*>)? ($($args:tt)*) $(-> $ret:ty)? $body:block
    ) => {
        $(#[$meta])*
        #[allow(dead_code)]
        $vis fn $name $(<$($gen),*>)? ($($args)*) $(-> $ret)? $body
    };
}

pub use allow_unused;

// ============================================================================
// Dead Code Elimination Prevention
// ============================================================================

/// Prevents linker from removing unused functions.
///
/// Use `#[used]` attribute on static items to prevent the linker from removing them,
/// even if they appear unused. This is critical for:
/// - Registration functions called via macros or type system
/// - FFI callbacks
/// - Interrupt handlers
/// - Static initializers
///
/// **Note:** `#[used]` works on statics, not functions directly. For functions,
/// use `#[no_mangle]` or call them from a `#[used]` static.
///
/// # Examples
///
/// ```
/// // Prevent removal of static data
/// #[used]
/// static PLUGIN_METADATA: &str = "plugin_v1.0";
///
/// // Registration via static
/// #[used]
/// static REGISTER: fn() = register_plugin;
///
/// fn register_plugin() {
///     println!("Plugin registered");
/// }
/// ```
///
/// For templated registration (C++ `ARCH_USED_FUNCTION` equivalent):
///
/// ```ignore
/// // Ensures registration happens even if not explicitly called
/// struct MyType;
///
/// impl MyType {
///     #[used]
///     fn __register() {
///         println!("MyType registered");
///     }
/// }
/// ```
///
/// Using macro helper:
///
/// ```ignore
/// use usd_arch::used_static;
///
/// used_static! {
///     static INIT: fn() = initialize_plugin;
/// }
///
/// fn initialize_plugin() {
///     println!("Initialized");
/// }
/// ```
#[macro_export]
macro_rules! used_static {
    (
        $(#[$meta:meta])*
        $vis:vis static $name:ident: $ty:ty = $value:expr;
    ) => {
        $(#[$meta])*
        #[used]
        $vis static $name: $ty = $value;
    };
}

pub use used_static;

/// Documents used function pattern.
///
/// In Rust, `#[used]` only applies to statics. For function preservation:
/// - Use `#[no_mangle]` for FFI functions
/// - Reference function in a `#[used]` static
/// - Use `ctor` crate for init/deinit functions
pub const USED_FUNCTION_NOTE: &str =
    "Rust #[used] applies to statics. Use #[no_mangle] for FFI or reference in #[used] static.";

// ============================================================================
// Static Constructors/Destructors
// ============================================================================

/// Documentation for static initialization patterns.
///
/// In Rust, use the `ctor` crate for C++-style constructor/destructor behavior.
///
/// # Setup
///
/// Add to `Cargo.toml`:
/// ```toml
/// [dependencies]
/// ctor = "0.2"
/// ```
///
/// # Examples
///
/// ```ignore
/// use ctor::{ctor, dtor};
///
/// #[ctor]
/// fn on_load() {
///     println!("Library loaded");
/// }
///
/// #[dtor]
/// fn on_unload() {
///     println!("Library unloaded");
/// }
/// ```
///
/// **Priority ordering:** The `ctor` crate doesn't support priority values.
/// For ordered initialization, use:
///
/// ```ignore
/// use std::sync::Once;
///
/// static INIT: Once = Once::new();
///
/// pub fn ensure_init() {
///     INIT.call_once(|| {
///         println!("Initialized once");
///     });
/// }
/// ```
///
/// Or use `lazy_static` / `once_cell`:
///
/// ```
/// use std::sync::OnceLock;
///
/// static RESOURCE: OnceLock<String> = OnceLock::new();
///
/// fn get_resource() -> &'static String {
///     RESOURCE.get_or_init(|| {
///         println!("Initializing resource");
///         String::from("initialized")
///     })
/// }
/// ```
pub const CONSTRUCTOR_NOTE: &str =
    "Use ctor crate: #[ctor] fn init() { }. For lazy init, use OnceLock or lazy_static.";

/// Note about destructor usage in Rust.
pub const DESTRUCTOR_NOTE: &str =
    "Use ctor crate: #[dtor] fn cleanup() { }. Or use Drop trait for RAII cleanup.";

/// Macro to document constructor usage.
///
/// ```ignore
/// arch_constructor! {
///     fn my_init() {
///         println!("Module initialized");
///     }
/// }
/// ```
///
/// Expands to documentation about using `ctor` crate.
#[macro_export]
macro_rules! arch_constructor {
    (fn $name:ident() $body:block) => {
        compile_error!(
            "Use ctor crate: #[ctor] fn init() { }. \
             Add ctor = \"0.2\" to Cargo.toml dependencies."
        );
    };
}

/// Macro to document destructor usage.
///
/// ```ignore
/// arch_destructor! {
///     fn my_cleanup() {
///         println!("Module cleaned up");
///     }
/// }
/// ```
///
/// Expands to documentation about using `ctor` crate.
#[macro_export]
macro_rules! arch_destructor {
    (fn $name:ident() $body:block) => {
        compile_error!(
            "Use ctor crate: #[dtor] fn cleanup() { }. \
             Add ctor = \"0.2\" to Cargo.toml dependencies."
        );
    };
}

// ============================================================================
// Empty Base Optimization
// ============================================================================

/// Documents empty base optimization.
///
/// **Not needed in Rust.** Rust automatically optimizes zero-sized types (ZSTs)
/// without requiring special attributes.
///
/// # Zero-Sized Types
///
/// Types with no data automatically have size 0:
///
/// ```
/// use std::mem::size_of;
///
/// struct Empty;
/// struct Marker<T>(std::marker::PhantomData<T>);
///
/// assert_eq!(size_of::<Empty>(), 0);
/// assert_eq!(size_of::<Marker<i32>>(), 0);
/// ```
///
/// # Composition
///
/// Unlike C++ empty base optimization, Rust ZSTs work everywhere:
///
/// ```
/// use std::mem::size_of;
///
/// struct Empty;
/// struct Container {
///     empty: Empty,
///     value: i32,
/// }
///
/// // Container is only 4 bytes (size of i32)
/// assert_eq!(size_of::<Container>(), 4);
/// ```
///
/// # PhantomData
///
/// Use `PhantomData` for type-level markers:
///
/// ```
/// use std::marker::PhantomData;
///
/// struct TypedHandle<T> {
///     id: u64,
///     _phantom: PhantomData<T>,
/// }
/// ```
pub const EMPTY_BASES_NOTE: &str =
    "Rust automatically optimizes zero-sized types. Use PhantomData for type markers.";

// ============================================================================
// Symbol Visibility and Export
// ============================================================================

/// Documents symbol export for dynamic libraries.
///
/// # FFI Export
///
/// For C-compatible FFI, use:
///
/// ```ignore
/// #[no_mangle]
/// pub extern "C" fn exported_function(x: i32) -> i32 {
///     x * 2
/// }
/// ```
///
/// # Dynamic Library Export
///
/// In `Cargo.toml`:
///
/// ```toml
/// [lib]
/// crate-type = ["cdylib"]  # For C-compatible dynamic library
/// # or
/// crate-type = ["dylib"]   # For Rust-only dynamic library
/// ```
///
/// # Visibility Control
///
/// Rust's module system controls visibility:
///
/// ```ignore
/// pub fn public_api() { }           // Public to all
/// pub(crate) fn internal_api() { }  // Public within crate
/// pub(super) fn parent_only() { }   // Public to parent module
/// fn private() { }                  // Private to module
/// ```
///
/// # Platform-Specific Export
///
/// ```no_run
/// #[cfg(windows)]
/// pub extern "C" fn windows_export() { }
///
/// #[cfg(unix)]
/// pub extern "C" fn unix_export() { }
/// ```
pub const EXPORT_NOTE: &str =
    "Use #[no_mangle] pub extern \"C\" for FFI exports. Set crate-type in Cargo.toml.";

/// Note about import usage in Rust.
pub const IMPORT_NOTE: &str =
    "Use extern \"C\" { } blocks to import C functions. No special attribute needed.";

/// Documents hidden symbol visibility.
///
/// **In Rust:** Use module privacy instead of symbol visibility attributes.
///
/// # Examples
///
/// ```
/// // Public API (exported)
/// pub mod api {
///     pub fn public_function() { }
/// }
///
/// // Internal implementation (hidden)
/// mod internal {
///     pub(crate) fn helper() { }  // Visible within crate
///     fn private() { }            // Completely private
/// }
/// ```
///
/// For FFI, control visibility via `pub`:
///
/// ```ignore
/// #[no_mangle]
/// pub extern "C" fn visible() { }
///
/// extern "C" fn hidden() { }  // Not exported
/// ```
pub const HIDDEN_NOTE: &str =
    "Use pub(crate) or module privacy. Rust's visibility system controls symbol export.";

// ============================================================================
// Fallthrough in Match
// ============================================================================

/// Documents match fallthrough behavior.
///
/// **Not needed in Rust.** Match arms in Rust are separate and don't fall through.
///
/// # Rust Match
///
/// Each arm is independent:
///
/// ```
/// let x = 5;
/// match x {
///     1 | 2 | 3 => println!("1-3"),  // Multiple patterns with |
///     4..=6 => println!("4-6"),       // Range pattern
///     _ => println!("other"),
/// }
/// ```
///
/// # Simulating Fallthrough
///
/// If you need shared logic:
///
/// ```
/// let x = 5;
/// match x {
///     1 | 2 | 3 => {
///         common_logic();
///     }
///     4 => {
///         common_logic();
///         extra_logic();
///     }
///     _ => {}
/// }
///
/// fn common_logic() {
///     println!("Common");
/// }
///
/// fn extra_logic() {
///     println!("Extra");
/// }
/// ```
///
/// Or use guards:
///
/// ```
/// let x = 5;
/// let needs_extra = x == 4;
///
/// match x {
///     1..=4 => {
///         println!("Common");
///         if needs_extra {
///             println!("Extra");
///         }
///     }
///     _ => {}
/// }
/// ```
pub const FALLTHROUGH_NOTE: &str =
    "Rust match arms don't fall through. Use | for multiple patterns or extract common logic.";

// ============================================================================
// Address Sanitizer
// ============================================================================

/// Documents address sanitizer control.
///
/// **In Rust:** Build with sanitizers enabled via rustc flags.
///
/// # Building with Sanitizers
///
/// ```bash
/// # Address sanitizer
/// RUSTFLAGS="-Z sanitizer=address" cargo build --target x86_64-unknown-linux-gnu
///
/// # Memory sanitizer
/// RUSTFLAGS="-Z sanitizer=memory" cargo build --target x86_64-unknown-linux-gnu
///
/// # Thread sanitizer
/// RUSTFLAGS="-Z sanitizer=thread" cargo build --target x86_64-unknown-linux-gnu
/// ```
///
/// # Disabling for Specific Functions
///
/// Rust doesn't have per-function sanitizer control. If needed:
/// 1. Extract to separate module/crate
/// 2. Use `#[cfg(not(sanitize))]` with custom build logic
/// 3. Use FFI to C code with `__attribute__((no_sanitize_address))`
///
/// # Unsafe Code and Sanitizers
///
/// Most Rust code doesn't need sanitizer exclusion because:
/// - Memory safety is enforced by the compiler
/// - Undefined behavior is limited to `unsafe` blocks
/// - Use Miri for testing unsafe code
///
/// ```bash
/// cargo +nightly miri test
/// ```
pub const NO_SANITIZE_ADDRESS_NOTE: &str =
    "Build with RUSTFLAGS=\"-Z sanitizer=address\". Use Miri for unsafe code testing.";

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    #[allow(unused_imports)]
    use super::*;

    #[test]
    fn test_inline_never_macro() {
        inline_never! {
            fn not_inlined(x: i32) -> i32 {
                x + 1
            }
        }
        assert_eq!(not_inlined(5), 6);
    }

    #[test]
    fn test_inline_always_macro() {
        inline_always! {
            fn always_inlined(x: i32) -> i32 {
                x * 2
            }
        }
        assert_eq!(always_inlined(5), 10);
    }

    #[test]
    fn test_allow_unused_macro() {
        allow_unused! {
            fn unused_function() -> i32 {
                42
            }
        }
        // Function compiles without warnings even if not called
    }

    #[test]
    fn test_used_static_macro() {
        used_static! {
            static TEST_VALUE: i32 = 100;
        }
        // Static won't be removed by linker
        assert_eq!(TEST_VALUE, 100);
    }

    #[test]
    fn test_zero_sized_types() {
        use std::mem::size_of;

        struct Empty;
        struct Container {
            _empty: Empty,
            _value: i32,
        }

        // Verify empty base optimization works automatically
        assert_eq!(size_of::<Empty>(), 0);
        assert_eq!(size_of::<Container>(), size_of::<i32>());
    }
}
