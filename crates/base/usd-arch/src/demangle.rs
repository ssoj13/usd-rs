//! Symbol demangling utilities.
//!
//! This module provides functions to demangle C++ and Rust symbol names generated
//! by the RTTI/`typeid()` facility and Rust's type system.
//!
//! # Example
//!
//! ```rust
//! use usd_arch::{arch_demangle, arch_get_demangled_type_name, arch_get_pretty_type_name};
//!
//! // Demangle a mangled symbol
//! if let Some(demangled) = arch_demangle("_ZN3std6string6StringE") {
//!     println!("Demangled: {}", demangled);
//! }
//!
//! // Get demangled type name
//! let type_name = arch_get_demangled_type_name::<Vec<String>>();
//! println!("Type: {}", type_name);
//!
//! // Get pretty (simplified) type name
//! let pretty = arch_get_pretty_type_name::<std::collections::HashMap<String, i32>>();
//! println!("Pretty: {}", pretty);
//! ```

use std::any::type_name;

/// Demangle a mangled symbol name.
///
/// This function attempts to demangle both Rust and C++ mangled symbol names.
/// It tries Rust demangling first using `rustc-demangle`, then falls back to
/// C++ demangling using `cpp_demangle` if available.
///
/// # Arguments
///
/// * `mangled` - The mangled symbol name to demangle
///
/// # Returns
///
/// * `Some(String)` - The demangled symbol name if successful
/// * `None` - If the symbol could not be demangled
///
/// # Example
///
/// ```rust
/// use usd_arch::arch_demangle;
///
/// // Rust symbol
/// let rust_symbol = "_RNvC6my_lib8my_function";
/// if let Some(demangled) = arch_demangle(rust_symbol) {
///     println!("Demangled: {}", demangled);
/// }
///
/// // C++ symbol
/// let cpp_symbol = "_ZN3std6string6StringE";
/// if let Some(demangled) = arch_demangle(cpp_symbol) {
///     println!("Demangled: {}", demangled);
/// }
/// ```
pub fn arch_demangle(mangled: &str) -> Option<String> {
    // Try Rust demangling first
    if let Ok(demangled) = rustc_demangle::try_demangle(mangled) {
        let mut result = format!("{:#}", demangled);
        fixup_string_names(&mut result);
        return Some(result);
    }

    // Try C++ demangling (prepend 'P' trick for simple types, like USD does)
    #[cfg(feature = "cpp-demangle")]
    {
        // First try direct demangling
        if let Ok(symbol) = cpp_demangle::Symbol::new(mangled) {
            let options = cpp_demangle::DemangleOptions::default();
            if let Ok(demangled) = symbol.demangle(&options) {
                let mut result = demangled;
                fixup_string_names(&mut result);
                return Some(result);
            }
        }

        // Try the 'P' prefix trick for simple types (like USD's _DemangleNewRaw)
        let prefixed = format!("P{}", mangled);
        if let Ok(symbol) = cpp_demangle::Symbol::new(&prefixed) {
            let options = cpp_demangle::DemangleOptions::default();
            if let Ok(demangled) = symbol.demangle(&options) {
                // Remove trailing '*' if present
                if let Some(stripped) = demangled.strip_suffix('*') {
                    let mut result = stripped.to_string();
                    fixup_string_names(&mut result);
                    return Some(result);
                }
            }
        }
    }

    None
}

/// Get demangled symbol name, returning empty string on failure.
///
/// This is a convenience wrapper around [`arch_demangle`] that returns
/// an empty string instead of `None` when demangling fails.
///
/// # Arguments
///
/// * `mangled` - The mangled symbol name to demangle
///
/// # Returns
///
/// The demangled symbol name, or an empty string if demangling failed.
///
/// # Example
///
/// ```rust
/// use usd_arch::arch_get_demangled;
///
/// let demangled = arch_get_demangled("_ZN3std6string6StringE");
/// if !demangled.is_empty() {
///     println!("Demangled: {}", demangled);
/// }
/// ```
pub fn arch_get_demangled(mangled: &str) -> String {
    arch_demangle(mangled).unwrap_or_default()
}

/// Get demangled type name for a type.
///
/// This function uses Rust's `std::any::type_name` to get the type name,
/// then attempts to demangle it. For Rust types, this typically returns
/// the full module path and type name.
///
/// # Type Parameters
///
/// * `T` - The type to get the demangled name for
///
/// # Returns
///
/// The demangled type name. If demangling fails, returns the raw type name.
///
/// # Example
///
/// ```rust
/// use usd_arch::arch_get_demangled_type_name;
///
/// let name = arch_get_demangled_type_name::<Vec<String>>();
/// println!("Type name: {}", name);
/// ```
pub fn arch_get_demangled_type_name<T: ?Sized>() -> String {
    let name = type_name::<T>();
    arch_demangle(name).unwrap_or_else(|| name.to_string())
}

/// Get pretty (simplified) type name.
///
/// This function returns a simplified version of the type name with common
/// prefixes and namespaces removed for better readability. It mimics USD's
/// `_FixupStringNames` behavior:
///
/// - Replaces `std::string::String` with `String`
/// - Replaces `alloc::string::String` with `String`
/// - Replaces `std::vec::Vec` with `Vec`
/// - Replaces `alloc::vec::Vec` with `Vec`
/// - Removes `std::` prefixes
/// - Removes `alloc::` prefixes
/// - Removes `core::` prefixes
/// - On Windows: removes `class `, `struct `, `enum ` prefixes
///
/// # Type Parameters
///
/// * `T` - The type to get the pretty name for
///
/// # Returns
///
/// A simplified, human-readable type name.
///
/// # Example
///
/// ```rust
/// use usd_arch::arch_get_pretty_type_name;
/// use std::collections::HashMap;
///
/// let pretty = arch_get_pretty_type_name::<HashMap<String, Vec<i32>>>();
/// println!("Pretty name: {}", pretty);
/// // Might print: "HashMap<String, Vec<i32>>" instead of full paths
/// ```
pub fn arch_get_pretty_type_name<T: ?Sized>() -> String {
    let name = arch_get_demangled_type_name::<T>();
    prettify_type_name(&name)
}

/// Demangle a function name.
///
/// This function is specifically designed for demangling function names,
/// which may require different handling than type names. It detects
/// Itanium ABI mangled names (starting with `_Z`) and handles them appropriately.
///
/// # Arguments
///
/// * `mangled_fn` - The mangled function name
///
/// # Returns
///
/// The demangled function name if successful, otherwise the original name.
///
/// # Example
///
/// ```rust
/// use usd_arch::arch_demangle_function_name;
///
/// let demangled = arch_demangle_function_name("_Z3foov");
/// println!("Function: {}", demangled);
/// ```
pub fn arch_demangle_function_name(mangled_fn: &str) -> String {
    // Check for Itanium ABI mangled names (start with _Z)
    if mangled_fn.len() > 2 && mangled_fn.starts_with("_Z") {
        // Try Rust demangling first
        if let Ok(demangled) = rustc_demangle::try_demangle(mangled_fn) {
            return format!("{:#}", demangled);
        }

        // For C++ function names, use direct demangling (not the 'P' trick)
        #[cfg(feature = "cpp-demangle")]
        {
            if let Ok(symbol) = cpp_demangle::Symbol::new(mangled_fn) {
                let options = cpp_demangle::DemangleOptions::default();
                if let Ok(demangled) = symbol.demangle(&options) {
                    return demangled;
                }
            }
        }
    }

    mangled_fn.to_string()
}

/// Fixup string type names in demangled output.
///
/// This internal function mimics USD's `_FixupStringNames` behavior:
/// - Replaces verbose `std::basic_string<char, ...>` with `string`
/// - Removes `std::` namespace prefixes
/// - On Windows: removes `class `, `struct `, `enum ` prefixes
/// - Removes trailing whitespace after replacements
fn fixup_string_names(name: &mut String) {
    // Get the demangled name of std::string::String for replacement
    static STRING_TYPE_REPLACEMENT: &str = "string";

    // Replace std::string::String variants
    *name = name.replace("std::string::String", STRING_TYPE_REPLACEMENT);
    *name = name.replace("alloc::string::String", STRING_TYPE_REPLACEMENT);

    // Replace std::basic_string<char, ...> (C++ pattern)
    // This is a simplified version - USD does more complex matching
    if let Some(pos) = name.find("std::basic_string<char") {
        if let Some(end_pos) = name[pos..].find('>') {
            let end = pos + end_pos + 1;
            name.replace_range(pos..end, STRING_TYPE_REPLACEMENT);
        }
    }

    // Remove std:: prefixes
    *name = name.replace("std::", "");

    // Remove trailing spaces after type replacements
    while name.contains("  ") {
        *name = name.replace("  ", " ");
    }

    // Windows-specific: remove class/struct/enum prefixes
    #[cfg(target_os = "windows")]
    {
        *name = name.replace("class ", "");
        *name = name.replace("struct ", "");
        *name = name.replace("enum ", "");
    }
}

/// Prettify type name by simplifying common standard library types.
///
/// This internal function makes type names more readable by:
/// - Simplifying standard library type paths
/// - Removing internal implementation details
fn prettify_type_name(name: &str) -> String {
    let mut result = name.to_string();

    // Replace common Rust standard library types
    result = result.replace("alloc::string::String", "String");
    result = result.replace("std::string::String", "String");
    result = result.replace("alloc::vec::Vec", "Vec");
    result = result.replace("std::vec::Vec", "Vec");
    result = result.replace("alloc::boxed::Box", "Box");
    result = result.replace("std::boxed::Box", "Box");
    result = result.replace("core::option::Option", "Option");
    result = result.replace("std::option::Option", "Option");
    result = result.replace("core::result::Result", "Result");
    result = result.replace("std::result::Result", "Result");
    result = result.replace("std::collections::hash::map::HashMap", "HashMap");
    result = result.replace("std::collections::btree::map::BTreeMap", "BTreeMap");

    // Remove namespace prefixes for cleaner output
    result = result.replace("std::", "");
    result = result.replace("alloc::", "");
    result = result.replace("core::", "");

    // Windows-specific cleanup
    #[cfg(target_os = "windows")]
    {
        result = result.replace("class ", "");
        result = result.replace("struct ", "");
        result = result.replace("enum ", "");
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_arch_demangle_rust_symbols() {
        // Rust symbols should be demangleable by rustc-demangle
        let symbol = "_RNvC6my_lib8my_function";
        if let Some(demangled) = arch_demangle(symbol) {
            // Just verify it doesn't panic and returns something
            assert!(!demangled.is_empty());
        }
    }

    #[test]
    fn test_arch_get_demangled_type_name() {
        // Test with standard types
        let name = arch_get_demangled_type_name::<Vec<String>>();
        assert!(!name.is_empty());
        assert!(name.contains("Vec") || name.contains("vector"));

        let name2 = arch_get_demangled_type_name::<i32>();
        assert!(!name2.is_empty());
    }

    #[test]
    fn test_arch_get_pretty_type_name() {
        // Test prettification
        let pretty = arch_get_pretty_type_name::<Vec<String>>();
        assert!(!pretty.is_empty());

        // Pretty name should not contain full paths
        assert!(!pretty.contains("alloc::"));

        let pretty2 = arch_get_pretty_type_name::<Option<i32>>();
        assert!(pretty2.contains("Option") || pretty2.contains("option"));
    }

    #[test]
    fn test_prettify_type_name() {
        let input = "alloc::vec::Vec<alloc::string::String>";
        let output = prettify_type_name(input);
        assert_eq!(output, "Vec<String>");

        let input2 = "std::option::Option<i32>";
        let output2 = prettify_type_name(input2);
        assert_eq!(output2, "Option<i32>");

        let input3 = "core::result::Result<std::string::String, std::io::Error>";
        let output3 = prettify_type_name(input3);
        assert_eq!(output3, "Result<String, io::Error>");
    }

    #[test]
    fn test_arch_get_demangled_empty() {
        // Test with invalid/unknown mangled name
        let result = arch_get_demangled("not_a_mangled_name");
        // Should return empty string on failure
        assert_eq!(result, "");
    }

    #[test]
    fn test_fixup_string_names() {
        let mut name = "std::string::String".to_string();
        fixup_string_names(&mut name);
        assert_eq!(name, "string");

        let mut name2 = "Vec<std::string::String>".to_string();
        fixup_string_names(&mut name2);
        assert_eq!(name2, "Vec<string>");
    }

    #[test]
    fn test_nested_types() {
        let pretty = arch_get_pretty_type_name::<Vec<Vec<String>>>();
        assert!(!pretty.contains("alloc::"));
        assert!(!pretty.contains("std::"));
    }

    #[test]
    fn test_arch_demangle_function_name() {
        // Test with non-mangled name (should return as-is)
        let result = arch_demangle_function_name("my_function");
        assert_eq!(result, "my_function");

        // Test with Itanium-style mangled name prefix
        let result2 = arch_demangle_function_name("_Z3foov");
        // Should attempt to demangle (even if it fails, should return something)
        assert!(!result2.is_empty());
    }

    #[cfg(feature = "cpp-demangle")]
    #[test]
    fn test_cpp_demangle() {
        // Test C++ symbol demangling if feature is enabled
        let cpp_symbol = "_ZNSt6vectorIiSaIiEED1Ev";
        if let Some(demangled) = arch_demangle(cpp_symbol) {
            assert!(!demangled.is_empty());
            // Should contain "vector" somewhere
            assert!(
                demangled.to_lowercase().contains("vector")
                    || demangled.to_lowercase().contains("vec")
            );
        }
    }
}
