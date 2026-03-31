//! Simple glob and regex pattern matching.
//!
//! Provides a pattern matcher that supports both glob-style patterns
//! and regular expressions.
//!
//! # Examples
//!
//! ```
//! use usd_tf::pattern_matcher::PatternMatcher;
//!
//! // Glob pattern matching
//! let matcher = PatternMatcher::new("*.txt", false, true);
//! assert!(matcher.matches("file.txt"));
//! assert!(!matcher.matches("file.rs"));
//!
//! // Case-insensitive matching
//! let matcher = PatternMatcher::new("HELLO*", false, true);
//! assert!(matcher.matches("helloworld"));
//! ```

use std::cell::RefCell;

use regex::Regex;

/// A pattern matcher for glob and regex patterns.
///
/// The pattern is compiled lazily on first use and cached.
/// Supports both glob patterns (with `*` and `?` wildcards) and
/// regular expression patterns.
#[derive(Debug)]
pub struct PatternMatcher {
    /// The pattern string.
    pattern: String,
    /// Whether matching is case-sensitive.
    case_sensitive: bool,
    /// Whether the pattern is a glob pattern.
    is_glob: bool,
    /// Compiled regex pattern (lazy).
    compiled: RefCell<Option<CompiledPattern>>,
}

/// Compiled pattern representation.
#[derive(Debug)]
struct CompiledPattern {
    /// The compiled regex, None if compilation failed.
    regex: Option<Regex>,
    /// Whether the pattern is valid.
    valid: bool,
    /// Error message if invalid.
    error: Option<String>,
}

impl PatternMatcher {
    /// Create a new pattern matcher.
    ///
    /// # Arguments
    ///
    /// * `pattern` - The pattern to match against
    /// * `case_sensitive` - Whether matching is case-sensitive
    /// * `is_glob` - Whether the pattern uses glob syntax
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_tf::pattern_matcher::PatternMatcher;
    ///
    /// let matcher = PatternMatcher::new("*.rs", false, true);
    /// assert!(matcher.matches("main.rs"));
    /// ```
    pub fn new(pattern: &str, case_sensitive: bool, is_glob: bool) -> Self {
        Self {
            pattern: pattern.to_string(),
            case_sensitive,
            is_glob,
            compiled: RefCell::new(None),
        }
    }

    /// Create an empty (invalid) pattern matcher.
    pub fn empty() -> Self {
        Self {
            pattern: String::new(),
            case_sensitive: false,
            is_glob: false,
            compiled: RefCell::new(None),
        }
    }

    /// Returns true if case-sensitive matching is enabled.
    #[inline]
    pub fn is_case_sensitive(&self) -> bool {
        self.case_sensitive
    }

    /// Returns true if glob pattern syntax is enabled.
    #[inline]
    pub fn is_glob_pattern(&self) -> bool {
        self.is_glob
    }

    /// Returns the pattern string.
    #[inline]
    pub fn pattern(&self) -> &str {
        &self.pattern
    }

    /// Returns true if the pattern is valid.
    ///
    /// Empty patterns are considered invalid.
    pub fn is_valid(&self) -> bool {
        if self.pattern.is_empty() {
            return false;
        }
        self.ensure_compiled();
        self.compiled.borrow().as_ref().is_some_and(|c| c.valid)
    }

    /// Returns the reason the pattern is invalid, if any.
    pub fn invalid_reason(&self) -> Option<String> {
        if self.pattern.is_empty() {
            return Some("empty pattern".to_string());
        }
        self.ensure_compiled();
        self.compiled
            .borrow()
            .as_ref()
            .and_then(|c| c.error.clone())
    }

    /// Set whether matching is case-sensitive.
    ///
    /// This invalidates any compiled pattern.
    pub fn set_case_sensitive(&mut self, sensitive: bool) {
        if self.case_sensitive != sensitive {
            self.case_sensitive = sensitive;
            *self.compiled.borrow_mut() = None;
        }
    }

    /// Set whether to use glob pattern syntax.
    ///
    /// This invalidates any compiled pattern.
    pub fn set_is_glob_pattern(&mut self, is_glob: bool) {
        if self.is_glob != is_glob {
            self.is_glob = is_glob;
            *self.compiled.borrow_mut() = None;
        }
    }

    /// Set the pattern string.
    ///
    /// This invalidates any compiled pattern.
    pub fn set_pattern(&mut self, pattern: &str) {
        if self.pattern != pattern {
            self.pattern = pattern.to_string();
            *self.compiled.borrow_mut() = None;
        }
    }

    /// Returns true if the query string matches the pattern.
    ///
    /// # Arguments
    ///
    /// * `query` - The string to match against the pattern
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_tf::pattern_matcher::PatternMatcher;
    ///
    /// let matcher = PatternMatcher::new("hello*", false, true);
    /// assert!(matcher.matches("hello world"));
    /// assert!(matcher.matches("world hello"));
    /// ```
    pub fn matches(&self, query: &str) -> bool {
        if !self.is_valid() {
            return false;
        }

        self.ensure_compiled();

        let compiled = self.compiled.borrow();
        let compiled = match compiled.as_ref() {
            Some(c) if c.valid => c,
            _ => return false,
        };

        compiled.regex.as_ref().is_some_and(|re| re.is_match(query))
    }

    /// Returns true if the query matches, with error message output.
    pub fn matches_with_error(&self, query: &str) -> (bool, Option<String>) {
        if !self.is_valid() {
            return (false, self.invalid_reason());
        }

        self.ensure_compiled();

        let compiled = self.compiled.borrow();
        let compiled = match compiled.as_ref() {
            Some(c) if c.valid => c,
            Some(c) => return (false, c.error.clone()),
            None => return (false, Some("pattern not compiled".to_string())),
        };

        let result = compiled.regex.as_ref().is_some_and(|re| re.is_match(query));
        (result, None)
    }

    /// Ensure the pattern is compiled.
    fn ensure_compiled(&self) {
        if self.compiled.borrow().is_none() {
            let compiled = self.compile();
            *self.compiled.borrow_mut() = Some(compiled);
        }
    }

    /// Compile the pattern.
    fn compile(&self) -> CompiledPattern {
        if self.pattern.is_empty() {
            return CompiledPattern {
                regex: None,
                valid: false,
                error: Some("empty pattern".to_string()),
            };
        }

        let regex_pattern = if self.is_glob {
            glob_to_regex(&self.pattern)
        } else {
            self.pattern.clone()
        };

        // Prepend (?i) flag for case-insensitive matching.
        let final_pattern = if !self.case_sensitive {
            format!("(?i){}", regex_pattern)
        } else {
            regex_pattern
        };

        match Regex::new(&final_pattern) {
            Ok(regex) => CompiledPattern {
                regex: Some(regex),
                valid: true,
                error: None,
            },
            Err(e) => CompiledPattern {
                regex: None,
                valid: false,
                error: Some(e.to_string()),
            },
        }
    }
}

impl Default for PatternMatcher {
    fn default() -> Self {
        Self::empty()
    }
}

impl Clone for PatternMatcher {
    fn clone(&self) -> Self {
        Self {
            pattern: self.pattern.clone(),
            case_sensitive: self.case_sensitive,
            is_glob: self.is_glob,
            compiled: RefCell::new(None), // Don't clone compiled state
        }
    }
}

/// Convert a glob pattern to a regex pattern.
///
/// Transforms (matching C++ TfStringGlobToRegex behaviour):
/// - `.` -> `\.`  (literal dot)
/// - `*` -> `.*`  (any sequence)
/// - `?` -> `.`   (any single char)
///
/// No `^`/`$` anchors are added — matching is unanchored (partial match),
/// consistent with the C++ implementation.
fn glob_to_regex(glob: &str) -> String {
    let mut regex = String::with_capacity(glob.len() * 2);

    for c in glob.chars() {
        match c {
            '*' => regex.push_str(".*"),
            '?' => regex.push('.'),
            '.' => regex.push_str("\\."),
            _ => regex.push(c),
        }
    }

    regex
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_pattern() {
        let matcher = PatternMatcher::empty();
        assert!(!matcher.is_valid());
        assert!(matcher.invalid_reason().is_some());
    }

    #[test]
    fn test_glob_exact_match() {
        let matcher = PatternMatcher::new("hello", false, true);
        assert!(matcher.matches("hello"));
        // Note: case_sensitive=false means case INSENSITIVE matching
        assert!(matcher.matches("Hello")); // Matches because case insensitive
        assert!(matcher.matches("HELLO"));
    }

    #[test]
    fn test_glob_case_insensitive() {
        let matcher = PatternMatcher::new("hello", false, true);
        assert!(matcher.matches("hello"));
        assert!(matcher.matches("HELLO"));
        assert!(matcher.matches("HeLLo"));
    }

    #[test]
    fn test_glob_case_sensitive() {
        let matcher = PatternMatcher::new("hello", true, true);
        assert!(matcher.matches("hello"));
        assert!(!matcher.matches("HELLO"));
        assert!(!matcher.matches("Hello"));
    }

    #[test]
    fn test_glob_star() {
        let matcher = PatternMatcher::new("*.txt", false, true);
        assert!(matcher.matches("file.txt"));
        assert!(matcher.matches("another.txt"));
        assert!(!matcher.matches("file.rs"));
    }

    #[test]
    fn test_glob_star_middle() {
        let matcher = PatternMatcher::new("hello*world", false, true);
        assert!(matcher.matches("helloworld"));
        assert!(matcher.matches("hello world"));
        assert!(matcher.matches("hello123world"));
        assert!(!matcher.matches("hello"));
    }

    #[test]
    fn test_glob_question() {
        let matcher = PatternMatcher::new("file?.txt", false, true);
        assert!(matcher.matches("file1.txt"));
        assert!(matcher.matches("fileA.txt"));
        assert!(!matcher.matches("file12.txt"));
        assert!(!matcher.matches("file.txt"));
    }

    #[test]
    fn test_glob_escape_dot() {
        let matcher = PatternMatcher::new("file.txt", false, true);
        assert!(matcher.matches("file.txt"));
        assert!(!matcher.matches("fileXtxt"));
    }

    #[test]
    fn test_glob_prefix() {
        // C++ glob matching is unanchored: "pre*" matches anywhere in the string.
        let matcher = PatternMatcher::new("pre*", false, true);
        assert!(matcher.matches("prefix"));
        assert!(matcher.matches("pre"));
        assert!(matcher.matches("prefiXXXXX"));
        // Unanchored: "xpre" also matches because "pre.*" finds "pre" inside it.
        assert!(matcher.matches("xpre"));

        // Regex pattern is also unanchored.
        let matcher = PatternMatcher::new("pre", false, false);
        assert!(matcher.matches("prefix"));
        assert!(matcher.matches("xpre"));
    }

    #[test]
    fn test_regex_anchored() {
        let matcher = PatternMatcher::new("^hello$", false, false);
        assert!(matcher.matches("hello"));
        assert!(!matcher.matches("hello world"));
        assert!(!matcher.matches("say hello"));
    }

    #[test]
    fn test_regex_unanchored() {
        let matcher = PatternMatcher::new("hello", false, false);
        assert!(matcher.matches("hello"));
        assert!(matcher.matches("hello world"));
        assert!(matcher.matches("say hello"));
    }

    #[test]
    fn test_set_pattern() {
        let mut matcher = PatternMatcher::new("old", false, true);
        assert!(matcher.matches("old"));
        assert!(!matcher.matches("new"));

        matcher.set_pattern("new");
        assert!(!matcher.matches("old"));
        assert!(matcher.matches("new"));
    }

    #[test]
    fn test_set_case_sensitive() {
        let mut matcher = PatternMatcher::new("hello", false, true);
        assert!(matcher.matches("HELLO"));

        matcher.set_case_sensitive(true);
        assert!(!matcher.matches("HELLO"));
        assert!(matcher.matches("hello"));
    }

    #[test]
    fn test_set_is_glob() {
        let mut matcher = PatternMatcher::new("*", false, true);
        assert!(matcher.matches("anything"));

        matcher.set_is_glob_pattern(false);
        // Now * is a regex quantifier, needs preceding element
        // Without anything to quantify, matching behavior changes
    }

    #[test]
    fn test_clone() {
        let matcher1 = PatternMatcher::new("*.txt", false, true);
        let matcher2 = matcher1.clone();

        assert_eq!(matcher1.pattern(), matcher2.pattern());
        assert_eq!(matcher1.is_case_sensitive(), matcher2.is_case_sensitive());
        assert_eq!(matcher1.is_glob_pattern(), matcher2.is_glob_pattern());
    }

    #[test]
    fn test_default() {
        let matcher = PatternMatcher::default();
        assert!(!matcher.is_valid());
        assert!(matcher.pattern().is_empty());
    }

    #[test]
    fn test_glob_to_regex() {
        // No anchors added — unanchored partial match, matching C++ behaviour.
        assert_eq!(glob_to_regex("*.txt"), ".*\\.txt");
        assert_eq!(glob_to_regex("file?.rs"), "file.\\.rs");
        assert_eq!(glob_to_regex("hello"), "hello");
        assert_eq!(glob_to_regex("a.b.c"), "a\\.b\\.c");
        assert_eq!(glob_to_regex("*hello*"), ".*hello.*");
        assert_eq!(glob_to_regex("*suffix"), ".*suffix");
        assert_eq!(glob_to_regex("prefix*"), "prefix.*");
    }

    #[test]
    fn test_matches_with_error() {
        let matcher = PatternMatcher::new("hello", false, true);
        let (result, error) = matcher.matches_with_error("hello");
        assert!(result);
        assert!(error.is_none());
    }

    #[test]
    fn test_matches_with_error_invalid() {
        let matcher = PatternMatcher::empty();
        let (result, error) = matcher.matches_with_error("hello");
        assert!(!result);
        assert!(error.is_some());
    }

    #[test]
    fn test_complex_glob() {
        let matcher = PatternMatcher::new("src/**/test_*.rs", false, true);
        // Note: Our simple implementation doesn't handle ** specially
        // It treats ** as * (zero or more of *) which effectively matches anything
        assert!(matcher.matches("src/foo/bar/test_main.rs"));
    }

    #[test]
    fn test_multiple_stars() {
        let matcher = PatternMatcher::new("*hello*world*", false, true);
        assert!(matcher.matches("XXhelloYYworldZZ"));
        assert!(matcher.matches("helloworld"));
        assert!(matcher.matches("hello world"));
    }
}
