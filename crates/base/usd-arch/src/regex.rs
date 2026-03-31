// Rust port: Copyright 2025
//

//! Regular expression matching utilities.
//!
//! This module provides a wrapper around the `regex` crate with USD-specific
//! functionality including glob pattern support and case-insensitive matching.
//!
//! # Examples
//!
//! ```
//! use usd_arch::ArchRegex;
//!
//! // Simple regex match
//! let re = ArchRegex::new("hello.*world", 0).unwrap();
//! assert!(re.match_str("hello beautiful world"));
//!
//! // Case-insensitive matching
//! let re = ArchRegex::new("Hello", ArchRegex::CASE_INSENSITIVE).unwrap();
//! assert!(re.match_str("hello"));
//!
//! // Glob pattern matching
//! let re = ArchRegex::new("*.txt", ArchRegex::GLOB).unwrap();
//! assert!(re.match_str("file.txt"));
//! ```

use regex::{Regex, RegexBuilder};

bitflags::bitflags! {
    /// Flags for regex compilation.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct RegexFlags: u32 {
        /// Case-insensitive matching.
        const CASE_INSENSITIVE = 1 << 0;
        /// Treat pattern as glob (convert *, ?, . to regex).
        const GLOB = 1 << 1;
    }
}

/// Regular expression matcher with USD-specific features.
///
/// Wraps the `regex` crate and adds support for:
/// - Glob patterns (`*`, `?` wildcards)
/// - Case-insensitive matching
/// - Error handling compatible with USD API
#[derive(Clone, Debug)]
pub struct ArchRegex {
    /// Compiled regex (None if compilation failed)
    regex: Option<Regex>,
    /// Compilation flags
    flags: RegexFlags,
    /// Error message (empty if successful)
    error: String,
}

impl ArchRegex {
    /// Flag for case-insensitive matching.
    pub const CASE_INSENSITIVE: u32 = RegexFlags::CASE_INSENSITIVE.bits();

    /// Flag for glob pattern matching.
    pub const GLOB: u32 = RegexFlags::GLOB.bits();

    /// Creates an empty, invalid regex.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_arch::ArchRegex;
    ///
    /// let re = ArchRegex::default();
    /// assert!(!re.is_valid());
    /// assert_eq!(re.get_error(), "uncompiled pattern");
    /// ```
    pub fn new_empty() -> Self {
        Self {
            regex: None,
            flags: RegexFlags::empty(),
            error: String::new(),
        }
    }

    /// Creates a regex from a pattern string with optional flags.
    ///
    /// # Arguments
    ///
    /// * `pattern` - Regular expression or glob pattern
    /// * `flags` - Combination of `CASE_INSENSITIVE` and `GLOB` flags
    ///
    /// # Returns
    ///
    /// `Ok(ArchRegex)` on success, `Err(String)` with error message on failure.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_arch::ArchRegex;
    ///
    /// // Regular regex
    /// let re = ArchRegex::new("test.*", 0).unwrap();
    /// assert!(re.match_str("testing"));
    ///
    /// // Glob pattern
    /// let re = ArchRegex::new("*.rs", ArchRegex::GLOB).unwrap();
    /// assert!(re.match_str("main.rs"));
    ///
    /// // Invalid pattern
    /// let re = ArchRegex::new("(unclosed", 0);
    /// assert!(re.is_err());
    /// ```
    pub fn new(pattern: &str, flags: u32) -> Result<Self, String> {
        let flags = RegexFlags::from_bits_truncate(flags);

        if pattern.is_empty() {
            return Err("empty pattern".to_string());
        }

        // Convert glob to regex if needed
        let pattern = if flags.contains(RegexFlags::GLOB) {
            glob_to_regex(pattern)
        } else {
            pattern.to_string()
        };

        // Build regex with flags
        let result = RegexBuilder::new(&pattern)
            .case_insensitive(flags.contains(RegexFlags::CASE_INSENSITIVE))
            // C++ uses REG_NEWLINE: '.' must NOT match '\n'
            .build();

        match result {
            Ok(regex) => Ok(Self {
                regex: Some(regex),
                flags,
                error: String::new(),
            }),
            Err(e) => Err(e.to_string()),
        }
    }

    /// Creates a regex, storing errors internally (USD-compatible API).
    ///
    /// Unlike `new()`, this never returns an error - failures are stored
    /// and can be queried with `is_valid()` and `get_error()`.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_arch::ArchRegex;
    ///
    /// let re = ArchRegex::from_pattern("(invalid", 0);
    /// assert!(!re.is_valid());
    /// assert!(!re.get_error().is_empty());
    /// ```
    pub fn from_pattern(pattern: &str, flags: u32) -> Self {
        match Self::new(pattern, flags) {
            Ok(regex) => regex,
            Err(error) => Self {
                regex: None,
                flags: RegexFlags::from_bits_truncate(flags),
                error,
            },
        }
    }

    /// Returns `true` if the regex compiled successfully.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_arch::ArchRegex;
    ///
    /// let valid = ArchRegex::new("test", 0).unwrap();
    /// assert!(valid.is_valid());
    ///
    /// let invalid = ArchRegex::from_pattern("(unclosed", 0);
    /// assert!(!invalid.is_valid());
    /// ```
    #[inline]
    pub fn is_valid(&self) -> bool {
        self.regex.is_some()
    }

    /// Returns the compilation error message, or empty string if valid.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_arch::ArchRegex;
    ///
    /// let re = ArchRegex::new("test", 0).unwrap();
    /// assert_eq!(re.get_error(), "");
    ///
    /// let re = ArchRegex::from_pattern("(invalid", 0);
    /// assert!(!re.get_error().is_empty());
    /// ```
    pub fn get_error(&self) -> &str {
        if self.regex.is_some() {
            ""
        } else if self.error.is_empty() {
            "uncompiled pattern"
        } else {
            &self.error
        }
    }

    /// Returns the flags used to create this regex.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_arch::ArchRegex;
    ///
    /// let re = ArchRegex::new("test", ArchRegex::CASE_INSENSITIVE).unwrap();
    /// assert_eq!(re.get_flags(), ArchRegex::CASE_INSENSITIVE);
    /// ```
    #[inline]
    pub fn get_flags(&self) -> u32 {
        self.flags.bits()
    }

    /// Tests if the pattern matches anywhere in the query string.
    ///
    /// Returns `false` if the regex is invalid.
    ///
    /// # Arguments
    ///
    /// * `query` - String to test against the pattern
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_arch::ArchRegex;
    ///
    /// let re = ArchRegex::new("world", 0).unwrap();
    /// assert!(re.match_str("hello world"));
    /// assert!(!re.match_str("hello there"));
    ///
    /// // Invalid regex never matches
    /// let re = ArchRegex::from_pattern("(invalid", 0);
    /// assert!(!re.match_str("anything"));
    /// ```
    pub fn match_str(&self, query: &str) -> bool {
        self.regex
            .as_ref()
            .map(|re| re.is_match(query))
            .unwrap_or(false)
    }

    /// Alternative name for USD C++ API compatibility.
    #[inline]
    pub fn r#match(&self, query: &str) -> bool {
        self.match_str(query)
    }
}

impl Default for ArchRegex {
    fn default() -> Self {
        Self::new_empty()
    }
}

/// Converts a glob pattern to a regular expression.
///
/// Transformations:
/// - `.` -> `\\.` (literal dot)
/// - `*` -> `.*` (zero or more of any character)
/// - `?` -> `.` (exactly one character)
///
/// # Examples
///
/// ```
/// # use usd_arch::glob_to_regex;
/// assert_eq!(glob_to_regex("*.txt"), ".*\\.txt");
/// assert_eq!(glob_to_regex("file?.rs"), "file.\\.rs");
/// ```
pub fn glob_to_regex(pattern: &str) -> String {
    let mut result = String::with_capacity(pattern.len() * 2);

    for ch in pattern.chars() {
        match ch {
            '.' => result.push_str("\\."),
            '*' => result.push_str(".*"),
            '?' => result.push('.'),
            // C++ _GlobToRegex only does 3 replacements (., *, ?)
            // All other chars pass through literally
            _ => result.push(ch),
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_pattern() {
        let re = ArchRegex::new("", 0);
        assert!(re.is_err());
        if let Err(e) = re {
            assert_eq!(e, "empty pattern");
        }

        let re = ArchRegex::from_pattern("", 0);
        assert!(!re.is_valid());
        assert_eq!(re.get_error(), "empty pattern");
    }

    #[test]
    fn test_default() {
        let re = ArchRegex::default();
        assert!(!re.is_valid());
        assert_eq!(re.get_error(), "uncompiled pattern");
        assert!(!re.match_str("anything"));
    }

    #[test]
    fn test_simple_match() {
        let re = ArchRegex::new("hello", 0).unwrap();
        assert!(re.is_valid());
        assert_eq!(re.get_error(), "");
        assert!(re.match_str("hello"));
        assert!(re.match_str("hello world"));
        assert!(re.match_str("say hello"));
        assert!(!re.match_str("goodbye"));
    }

    #[test]
    fn test_regex_pattern() {
        let re = ArchRegex::new("test.*world", 0).unwrap();
        assert!(re.match_str("test 123 world"));
        assert!(re.match_str("testing world"));
        assert!(!re.match_str("test"));
        assert!(!re.match_str("world"));
    }

    #[test]
    fn test_case_insensitive() {
        let re = ArchRegex::new("Hello", ArchRegex::CASE_INSENSITIVE).unwrap();
        assert_eq!(re.get_flags(), ArchRegex::CASE_INSENSITIVE);
        assert!(re.match_str("hello"));
        assert!(re.match_str("HELLO"));
        assert!(re.match_str("HeLLo"));
        assert!(re.match_str("say hello there"));
    }

    #[test]
    fn test_case_sensitive() {
        let re = ArchRegex::new("Hello", 0).unwrap();
        assert_eq!(re.get_flags(), 0);
        assert!(re.match_str("Hello"));
        assert!(!re.match_str("hello"));
        assert!(!re.match_str("HELLO"));
    }

    #[test]
    fn test_glob_pattern() {
        let re = ArchRegex::new("*.txt", ArchRegex::GLOB).unwrap();
        assert!(re.match_str("file.txt"));
        assert!(re.match_str("test.txt"));
        assert!(re.match_str("path/to/file.txt"));
        assert!(!re.match_str("file.rs"));
    }

    #[test]
    fn test_glob_question_mark() {
        let re = ArchRegex::new("file?.rs", ArchRegex::GLOB).unwrap();
        assert!(re.match_str("file1.rs"));
        assert!(re.match_str("fileA.rs"));
        assert!(!re.match_str("file.rs"));
        assert!(!re.match_str("file12.rs"));
    }

    #[test]
    fn test_glob_with_dots() {
        let re = ArchRegex::new("test.*", ArchRegex::GLOB).unwrap();
        assert!(re.match_str("test.txt"));
        assert!(re.match_str("test.anything"));
        // Dot is literal in glob
        assert!(!re.match_str("testabc"));
    }

    #[test]
    fn test_glob_case_insensitive() {
        let flags = ArchRegex::GLOB | ArchRegex::CASE_INSENSITIVE;
        let re = ArchRegex::new("*.TXT", flags).unwrap();
        assert!(re.match_str("file.txt"));
        assert!(re.match_str("FILE.TXT"));
        assert!(re.match_str("Test.Txt"));
    }

    #[test]
    fn test_invalid_pattern() {
        let re = ArchRegex::new("(unclosed", 0);
        assert!(re.is_err());

        let re = ArchRegex::from_pattern("(unclosed", 0);
        assert!(!re.is_valid());
        assert!(!re.get_error().is_empty());
        assert!(!re.match_str("anything"));
    }

    #[test]
    fn test_clone() {
        let re1 = ArchRegex::new("test", 0).unwrap();
        let re2 = re1.clone();
        assert!(re2.is_valid());
        assert!(re2.match_str("test"));
        assert_eq!(re1.get_flags(), re2.get_flags());
    }

    #[test]
    fn test_glob_to_regex_fn() {
        assert_eq!(glob_to_regex("*.txt"), ".*\\.txt");
        assert_eq!(glob_to_regex("file?.rs"), "file.\\.rs");
        assert_eq!(glob_to_regex("test.*"), "test\\..*");
        assert_eq!(glob_to_regex("a?b*c.d"), "a.b.*c\\.d");
    }

    #[test]
    fn test_glob_special_chars() {
        // C++ _GlobToRegex does NOT escape extra chars — they pass through
        // as regex metacharacters. [1] is a character class matching '1'.
        let re = ArchRegex::new("test[1].txt", ArchRegex::GLOB).unwrap();
        // [1] matches '1', . is escaped to \., so matches "test1.txt"
        assert!(re.match_str("test1.txt"));
    }

    #[test]
    fn test_match_alias() {
        let re = ArchRegex::new("test", 0).unwrap();
        assert!(re.r#match("test"));
        assert!(!re.r#match("other"));
    }

    #[test]
    fn test_complex_glob() {
        // Note: ** is not standard glob, just treat as *
        let re = ArchRegex::new("src/*/*.rs", ArchRegex::GLOB).unwrap();
        assert!(re.match_str("src/lib/test.rs"));
        assert!(re.match_str("src/foo/bar.rs"));
        assert!(!re.match_str("src/file.txt"));
    }

    #[test]
    fn test_anchored_pattern() {
        // Rust regex crate needs explicit anchoring
        let re = ArchRegex::new("^hello$", 0).unwrap();
        assert!(re.match_str("hello"));
        assert!(!re.match_str("hello world"));
        assert!(!re.match_str("say hello"));
    }

    #[test]
    fn test_unicode() {
        let re = ArchRegex::new("тест", 0).unwrap();
        assert!(re.match_str("тест"));
        assert!(re.match_str("это тест"));

        let re = ArchRegex::new("тест", ArchRegex::CASE_INSENSITIVE).unwrap();
        assert!(re.match_str("ТЕСТ"));
    }

    #[test]
    fn test_newline_handling() {
        // C++ REG_NEWLINE: '.' does NOT match '\n'
        let re = ArchRegex::new("hello.*world", 0).unwrap();
        // '.*' should NOT cross newline boundaries
        assert!(!re.match_str("hello\nworld"));
        // Same line still works
        assert!(re.match_str("hello beautiful world"));
    }
}
