//! Function name formatting utilities.
//!
//! This module provides functions for formatting and prettifying function names.
//! This is the Rust equivalent of `pxr/base/arch/function.{cpp,h}`.
//!
//! # Example
//!
//! ```ignore
//! use usd_arch::{arch_function, arch_pretty_function, arch_get_prettier_function_name};
//!
//! fn my_function() {
//!     let function = arch_function!();
//!     let pretty = arch_pretty_function!();
//!     let prettier = arch_get_prettier_function_name(function, pretty);
//!     println!("Prettier function name: {}", prettier);
//! }
//! ```

use std::collections::HashMap;

/// Returns a well-formatted function name.
///
/// This function takes a simple function name and a "pretty" function name
/// (with full signature) and attempts to reconstruct a well-formatted function name.
///
/// This is a Rust adaptation of the C++ `ArchGetPrettierFunctionName` function.
/// Since Rust's type system and function signatures differ from C++, this
/// implementation focuses on extracting meaningful function names from Rust's
/// `type_name` format.
///
/// # Arguments
///
/// * `function` - Simple function name (from `arch_function!()`)
/// * `pretty_function` - Full function signature (from `arch_pretty_function!()`)
///
/// # Returns
///
/// A formatted function name string.
///
/// # Example
///
/// ```ignore
/// use usd_arch::{arch_function, arch_pretty_function, arch_get_prettier_function_name};
///
/// fn test_function() {
///     let func = arch_function!();
///     let pretty = arch_pretty_function!();
///     let prettier = arch_get_prettier_function_name(func, pretty);
///     println!("{}", prettier);
/// }
/// ```
pub fn arch_get_prettier_function_name(function: &str, pretty_function: &str) -> String {
    // Get the function signature and template list, respectively.
    let (func_part, template_part) = split(pretty_function);

    // Get just the function name.
    let function_name = get_function_name(function, &func_part);

    // Get the types from the template list.
    let mut template_list = get_template_list(&template_part);

    // Discard types from the template list that aren't in function_name.
    template_list = filter_template_list(&function_name, &template_list);

    // Construct the prettier function name.
    format!("{}{}", function_name, format_template_list(&template_list))
}

// Helper functions for parsing (adapted from C++ implementation)

/// Returns the start of the type name in a string that ends at position `i`.
fn get_start_of_name(s: &str, i: usize) -> usize {
    if i >= s.len() {
        return 0;
    }

    let mut pos = i;
    let chars: Vec<char> = s.chars().collect();

    // Skip backwards until we find the start of the function name
    // Skip over everything between matching '<' and '>'
    while pos > 0 {
        if let Some(ch) = chars.get(pos) {
            if *ch == ' ' || *ch == '>' {
                let mut nesting_depth = 1;
                let mut search_pos = pos;

                // Skip over template parameters
                while nesting_depth > 0 && search_pos > 0 {
                    search_pos -= 1;
                    if let Some(c) = chars.get(search_pos) {
                        if *c == '>' {
                            nesting_depth += 1;
                        } else if *c == '<' {
                            nesting_depth -= 1;
                        }
                    }
                }

                // Find the space before the template
                while search_pos > 0 {
                    if let Some(c) = chars.get(search_pos) {
                        if *c == ' ' {
                            return search_pos + 1;
                        }
                    }
                    search_pos -= 1;
                }

                return 0;
            }
        }

        if pos == 0 {
            break;
        }
        pos -= 1;
    }

    0
}

/// Finds the real name of function in pretty_function.
fn get_function_name(function: &str, pretty_function: &str) -> String {
    // Prepend "::" to search for member function
    let member_function = format!("::{}", function);

    // First search to see if function is a member function
    if let Some(function_start) = pretty_function.find(&member_function) {
        if function_start > 0 {
            let function_end = function_start + function.len() + 2;

            // Find the start of the function name
            let name_start = get_start_of_name(pretty_function, function_start);

            // Extract the function name
            if name_start < function_end && function_end <= pretty_function.len() {
                return pretty_function[name_start..function_end].to_string();
            }
        }
    }

    // If not found as member function, return the function name as-is
    function.to_string()
}

/// Splits pretty_function into function part and template list part.
fn split(pretty_function: &str) -> (String, String) {
    // " [with " is 7 characters
    const WITH_PREFIX_LEN: usize = 7;
    if let Some(i) = pretty_function.find(" [with ") {
        let n = pretty_function.len();
        let func_part = pretty_function[..i].to_string();
        let template_part = pretty_function[i + WITH_PREFIX_LEN..n - 1].to_string();
        (func_part, template_part)
    } else {
        (pretty_function.to_string(), String::new())
    }
}

/// Splits template list into a map.
fn get_template_list(templates: &str) -> HashMap<String, String> {
    let mut result = HashMap::new();

    if templates.is_empty() {
        return result;
    }

    // Parse template assignments like "A = int, B = float"
    let parts: Vec<&str> = templates.split(',').collect();

    for part in parts {
        let part = part.trim();
        if let Some(eq_pos) = part.find('=') {
            let name = part[..eq_pos].trim().to_string();
            let value = part[eq_pos + 1..].trim().to_string();
            result.insert(name, value);
        }
    }

    result
}

/// Formats template list as a string.
fn format_template_list(templates: &HashMap<String, String>) -> String {
    if templates.is_empty() {
        return String::new();
    }

    let mut result = String::from(" [with ");
    let mut first = true;

    for (name, value) in templates {
        if !first {
            result.push_str(", ");
        }
        result.push_str(name);
        result.push_str(" = ");
        result.push_str(value);
        first = false;
    }

    result.push(']');
    result
}

/// Filters template list to only include templates found in pretty_function.
fn filter_template_list(
    pretty_function: &str,
    templates: &HashMap<String, String>,
) -> HashMap<String, String> {
    let mut result = HashMap::new();

    if let Some(pos) = pretty_function.find('<') {
        // Extract template parameter names from the function signature
        // This is a simplified version - full implementation would parse
        // the template parameter list more carefully
        let template_part = &pretty_function[pos + 1..];

        for (name, value) in templates {
            if template_part.contains(name) {
                result.insert(name.clone(), value.clone());
            }
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_arch_get_prettier_function_name() {
        let function = "test_function";
        let pretty = "crate::module::test_function::f";
        let result = arch_get_prettier_function_name(function, pretty);
        assert!(result.contains("test_function"));
    }

    #[test]
    fn test_get_function_name() {
        let function = "Bar";
        let pretty = "int Foo<A>::Bar () [with A = int]";
        let result = get_function_name(function, pretty);
        assert!(result.contains("Bar"));
    }

    #[test]
    fn test_split() {
        let pretty = "int Foo<A>::Bar () [with A = int]";
        let (func, template) = split(pretty);
        assert_eq!(func, "int Foo<A>::Bar ()");
        assert_eq!(template, "A = int");
    }

    #[test]
    fn test_get_template_list() {
        let templates = "A = int, B = float";
        let map = get_template_list(templates);
        assert_eq!(map.get("A"), Some(&"int".to_string()));
        assert_eq!(map.get("B"), Some(&"float".to_string()));
    }

    #[test]
    fn test_format_template_list() {
        let mut map = HashMap::new();
        map.insert("A".to_string(), "int".to_string());
        map.insert("B".to_string(), "float".to_string());
        let formatted = format_template_list(&map);
        assert!(formatted.contains("A = int"));
        assert!(formatted.contains("B = float"));
    }
}
