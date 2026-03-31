//! Conditional abort diagnostic delegate.
//!
//! Provides a diagnostic delegate that can abort operations based on
//! include/exclude filter rules applied to errors and warnings.

use usd_tf::CallContext;

/// Error filters for the conditional abort delegate.
#[derive(Debug, Clone, Default)]
pub struct ConditionalAbortDiagnosticDelegateErrorFilters {
    /// Filters matching on error/warning text.
    string_filters: Vec<String>,
    /// Filters matching on code path.
    code_path_filters: Vec<String>,
}

impl ConditionalAbortDiagnosticDelegateErrorFilters {
    /// Creates a new empty filter set.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a new filter set with the given filters.
    pub fn with_filters(string_filters: Vec<String>, code_path_filters: Vec<String>) -> Self {
        Self {
            string_filters,
            code_path_filters,
        }
    }

    /// Returns the string filters.
    pub fn get_string_filters(&self) -> &[String] {
        &self.string_filters
    }

    /// Returns the code path filters.
    pub fn get_code_path_filters(&self) -> &[String] {
        &self.code_path_filters
    }

    /// Sets the string filters.
    pub fn set_string_filters(&mut self, filters: Vec<String>) {
        self.string_filters = filters;
    }

    /// Sets the code path filters.
    pub fn set_code_path_filters(&mut self, filters: Vec<String>) {
        self.code_path_filters = filters;
    }

    /// Returns true if no filters are defined.
    pub fn is_empty(&self) -> bool {
        self.string_filters.is_empty() && self.code_path_filters.is_empty()
    }
}

/// Compiled pattern matchers for efficient pattern matching.
struct CompiledFilters {
    string_patterns: Vec<String>,
    code_path_patterns: Vec<String>,
}

impl CompiledFilters {
    fn from_filters(filters: &ConditionalAbortDiagnosticDelegateErrorFilters) -> Self {
        Self {
            string_patterns: filters.string_filters.clone(),
            code_path_patterns: filters.code_path_filters.clone(),
        }
    }

    fn matches(&self, text: &str, code_path: &str) -> bool {
        // Simple glob-style matching (uses * as wildcard)
        for pattern in &self.string_patterns {
            if glob_match(pattern, text) {
                return true;
            }
        }

        for pattern in &self.code_path_patterns {
            if glob_match(pattern, code_path) {
                return true;
            }
        }

        false
    }
}

/// Shell glob-style pattern matching (case-insensitive).
///
/// Supports '*' wildcard only. Matches C++ TfPatternMatcher behavior used
/// by the conditional abort diagnostic delegate.
fn glob_match(pattern: &str, text: &str) -> bool {
    if pattern.is_empty() {
        return text.is_empty();
    }

    // Case-insensitive matching like C++ TfPatternMatcher(filter, true, true)
    let pattern = pattern.to_lowercase();
    let text = text.to_lowercase();

    let parts: Vec<&str> = pattern.split('*').collect();

    if parts.len() == 1 {
        // No wildcard — exact match
        return pattern == text;
    }

    let mut remaining: &str = &text;

    // First part must anchor at the start
    let first = parts[0];
    if !first.is_empty() {
        if !remaining.starts_with(first) {
            return false;
        }
        remaining = &remaining[first.len()..];
    }

    // Last part must anchor at the end
    let last = parts[parts.len() - 1];
    let end_reserved = if !last.is_empty() {
        if !remaining.ends_with(last) {
            return false;
        }
        // Reserve suffix so intermediate parts can't consume it
        last.len()
    } else {
        0
    };

    // Intermediate parts must be found in order within the remaining range
    let search_region = &remaining[..remaining.len() - end_reserved];
    let mut cursor = search_region;
    for part in &parts[1..parts.len() - 1] {
        if part.is_empty() {
            continue;
        }
        if let Some(pos) = cursor.find(part) {
            cursor = &cursor[pos + part.len()..];
        } else {
            return false;
        }
    }

    true
}

/// A diagnostic delegate that conditionally aborts on errors/warnings.
pub struct ConditionalAbortDiagnosticDelegate {
    /// Compiled include filters.
    include_filters: CompiledFilters,
    /// Compiled exclude filters.
    exclude_filters: CompiledFilters,
    /// Whether to actually abort (can be disabled for testing).
    abort_enabled: bool,
}

impl ConditionalAbortDiagnosticDelegate {
    /// Creates a new conditional abort delegate.
    pub fn new(
        include_filters: ConditionalAbortDiagnosticDelegateErrorFilters,
        exclude_filters: ConditionalAbortDiagnosticDelegateErrorFilters,
    ) -> Self {
        Self {
            include_filters: CompiledFilters::from_filters(&include_filters),
            exclude_filters: CompiledFilters::from_filters(&exclude_filters),
            abort_enabled: true,
        }
    }

    /// Creates a delegate with abort disabled (for testing).
    pub fn new_test_mode(
        include_filters: ConditionalAbortDiagnosticDelegateErrorFilters,
        exclude_filters: ConditionalAbortDiagnosticDelegateErrorFilters,
    ) -> Self {
        Self {
            include_filters: CompiledFilters::from_filters(&include_filters),
            exclude_filters: CompiledFilters::from_filters(&exclude_filters),
            abort_enabled: false,
        }
    }

    /// Issues an error diagnostic.
    pub fn issue_error(&self, context: &CallContext, message: &str) {
        self.check_and_abort(context, message);
    }

    /// Issues a warning diagnostic.
    pub fn issue_warning(&self, context: &CallContext, message: &str) {
        self.check_and_abort(context, message);
    }

    /// Issues a fatal error diagnostic.
    pub fn issue_fatal_error(&self, context: &CallContext, msg: &str) {
        eprintln!(
            "FATAL ERROR at {}:{} in {}: {}",
            context.file(),
            context.line(),
            context.function(),
            msg
        );

        if self.abort_enabled {
            std::process::abort();
        }
    }

    /// Issues a status diagnostic.
    pub fn issue_status(&self, _context: &CallContext, _message: &str) {
        // Status messages don't trigger abort
    }

    /// Checks if a diagnostic should trigger an abort.
    fn check_and_abort(&self, context: &CallContext, text: &str) {
        let code_path = format!(
            "{}:{}:{}",
            context.file(),
            context.line(),
            context.function()
        );

        if !self.include_filters.matches(text, &code_path) {
            return;
        }

        if self.exclude_filters.matches(text, &code_path) {
            return;
        }

        eprintln!(
            "Conditional abort triggered at {}:{} in {}: {}",
            context.file(),
            context.line(),
            context.function(),
            text
        );

        if self.abort_enabled {
            std::process::abort();
        }
    }

    /// Returns true if abort is enabled.
    pub fn is_abort_enabled(&self) -> bool {
        self.abort_enabled
    }

    /// Tests if an error/warning would trigger an abort.
    pub fn would_abort(&self, text: &str, code_path: &str) -> bool {
        if !self.include_filters.matches(text, code_path) {
            return false;
        }

        if self.exclude_filters.matches(text, code_path) {
            return false;
        }

        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_filters_default() {
        let filters = ConditionalAbortDiagnosticDelegateErrorFilters::new();
        assert!(filters.is_empty());
    }

    #[test]
    fn test_error_filters_with_values() {
        let filters = ConditionalAbortDiagnosticDelegateErrorFilters::with_filters(
            vec!["error*".to_string()],
            vec!["pxr::*".to_string()],
        );

        assert_eq!(filters.get_string_filters().len(), 1);
        assert_eq!(filters.get_code_path_filters().len(), 1);
    }

    #[test]
    fn test_glob_match() {
        assert!(glob_match("*error*", "This is an error message"));
        assert!(glob_match("hello*", "hello world"));
        assert!(glob_match("*world", "hello world"));
        assert!(glob_match("*", "anything"));
        assert!(!glob_match("hello", "world"));

        // Case-insensitive (matches C++ TfPatternMatcher behavior)
        assert!(glob_match("*ERROR*", "this is an error message"));
        assert!(glob_match("Hello*", "hello world"));

        // Multiple wildcards with intermediate parts
        assert!(glob_match("a*b*c", "axbxc"));
        assert!(glob_match("a*b*c", "aXXbYYc"));
        assert!(!glob_match("a*b*c", "axcxb"));

        // Greedy scan fix: end part must not be consumed by intermediate scan
        assert!(glob_match("*foo*bar", "xfooxbar"));
        assert!(!glob_match("*foo*bar", "xfooxbaz"));

        // Empty and exact
        assert!(glob_match("", ""));
        assert!(!glob_match("", "x"));
        assert!(glob_match("exact", "exact"));
        assert!(!glob_match("exact", "EXACT_not"));
    }

    #[test]
    fn test_would_abort_include_match() {
        let include = ConditionalAbortDiagnosticDelegateErrorFilters::with_filters(
            vec!["*error*".to_string()],
            vec![],
        );
        let exclude = ConditionalAbortDiagnosticDelegateErrorFilters::new();

        let delegate = ConditionalAbortDiagnosticDelegate::new_test_mode(include, exclude);

        assert!(delegate.would_abort("This is an error message", "some/path"));
        assert!(!delegate.would_abort("This is a warning", "some/path"));
    }

    #[test]
    fn test_would_abort_exclude_match() {
        let include = ConditionalAbortDiagnosticDelegateErrorFilters::with_filters(
            vec!["*error*".to_string()],
            vec![],
        );
        let exclude = ConditionalAbortDiagnosticDelegateErrorFilters::with_filters(
            vec!["*test*".to_string()],
            vec![],
        );

        let delegate = ConditionalAbortDiagnosticDelegate::new_test_mode(include, exclude);

        assert!(!delegate.would_abort("test error message", "some/path"));
        assert!(delegate.would_abort("production error message", "some/path"));
    }
}
