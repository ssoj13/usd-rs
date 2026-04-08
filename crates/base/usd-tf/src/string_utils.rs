//! String utilities for USD.
//!
//! This module provides various string manipulation functions commonly
//! used throughout USD. Many wrap Rust's built-in string methods but
//! provide a consistent API matching the C++ USD library.
//!
//! # Examples
//!
//! ```
//! use usd_tf::string_utils::*;
//!
//! // String operations
//! assert!(starts_with("hello world", "hello"));
//! assert!(ends_with("hello world", "world"));
//! assert_eq!(to_lower("HELLO"), "hello");
//! assert_eq!(trim("  hello  "), "hello");
//!
//! // Splitting and joining
//! let parts = tokenize("a b c", " ");
//! assert_eq!(parts, vec!["a", "b", "c"]);
//!
//! let joined = join(&["a", "b", "c"], ", ");
//! assert_eq!(joined, "a, b, c");
//! ```

use std::collections::HashSet;

/// Returns true if `s` starts with `prefix`.
///
/// # Examples
///
/// ```
/// use usd_tf::string_utils::starts_with;
///
/// assert!(starts_with("hello world", "hello"));
/// assert!(!starts_with("hello world", "world"));
/// ```
#[inline]
#[must_use]
pub fn starts_with(s: &str, prefix: &str) -> bool {
    s.starts_with(prefix)
}

/// Returns true if `s` ends with `suffix`.
///
/// # Examples
///
/// ```
/// use usd_tf::string_utils::ends_with;
///
/// assert!(ends_with("hello world", "world"));
/// assert!(!ends_with("hello world", "hello"));
/// ```
#[inline]
#[must_use]
pub fn ends_with(s: &str, suffix: &str) -> bool {
    s.ends_with(suffix)
}

/// Returns true if `s` contains `substring`.
///
/// # Examples
///
/// ```
/// use usd_tf::string_utils::contains;
///
/// assert!(contains("hello world", "lo wo"));
/// assert!(!contains("hello world", "xyz"));
/// ```
#[inline]
#[must_use]
pub fn contains(s: &str, substring: &str) -> bool {
    s.contains(substring)
}

/// Convert string to lowercase.
///
/// # Examples
///
/// ```
/// use usd_tf::string_utils::to_lower;
///
/// assert_eq!(to_lower("HELLO World"), "hello world");
/// ```
#[inline]
#[must_use]
pub fn to_lower(s: &str) -> String {
    s.to_lowercase()
}

/// Convert string to uppercase.
///
/// # Examples
///
/// ```
/// use usd_tf::string_utils::to_upper;
///
/// assert_eq!(to_upper("hello World"), "HELLO WORLD");
/// ```
#[inline]
#[must_use]
pub fn to_upper(s: &str) -> String {
    s.to_uppercase()
}

/// Convert ASCII characters to lowercase (locale-independent).
///
/// Only ASCII letters [A-Z] are converted to lowercase.
/// Non-ASCII characters are left unchanged.
///
/// # Examples
///
/// ```
/// use usd_tf::string_utils::to_lower_ascii;
///
/// assert_eq!(to_lower_ascii("HELLO"), "hello");
/// assert_eq!(to_lower_ascii("Über"), "Über"); // Non-ASCII unchanged
/// ```
#[must_use]
pub fn to_lower_ascii(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_ascii_uppercase() {
                c.to_ascii_lowercase()
            } else {
                c
            }
        })
        .collect()
}

/// Capitalize the first character of the string.
///
/// Returns a copy with only the first character uppercased.
/// The rest of the string is left unchanged (matches C++ TfStringCapitalize).
///
/// # Examples
///
/// ```
/// use usd_tf::string_utils::capitalize;
///
/// assert_eq!(capitalize("hello"), "Hello");
/// assert_eq!(capitalize("HELLO"), "HELLO");
/// assert_eq!(capitalize(""), "");
/// ```
#[must_use]
pub fn capitalize(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(first) => {
            let mut result = first.to_uppercase().to_string();
            result.push_str(chars.as_str());
            result
        }
    }
}

/// Trim whitespace from both ends of the string.
///
/// # Examples
///
/// ```
/// use usd_tf::string_utils::trim;
///
/// assert_eq!(trim("  hello  "), "hello");
/// assert_eq!(trim("\t\nhello\r\n"), "hello");
/// ```
#[inline]
#[must_use]
pub fn trim(s: &str) -> &str {
    s.trim()
}

/// Trim specified characters from both ends of the string.
///
/// # Examples
///
/// ```
/// use usd_tf::string_utils::trim_chars;
///
/// assert_eq!(trim_chars("...hello...", "."), "hello");
/// assert_eq!(trim_chars("xxhelloxx", "x"), "hello");
/// ```
#[must_use]
pub fn trim_chars<'a>(s: &'a str, chars: &str) -> &'a str {
    let char_set: HashSet<char> = chars.chars().collect();
    s.trim_matches(|c| char_set.contains(&c))
}

/// Trim whitespace from the left of the string.
///
/// # Examples
///
/// ```
/// use usd_tf::string_utils::trim_left;
///
/// assert_eq!(trim_left("  hello  "), "hello  ");
/// ```
#[inline]
#[must_use]
pub fn trim_left(s: &str) -> &str {
    s.trim_start()
}

/// Trim whitespace from the right of the string.
///
/// # Examples
///
/// ```
/// use usd_tf::string_utils::trim_right;
///
/// assert_eq!(trim_right("  hello  "), "  hello");
/// ```
#[inline]
#[must_use]
pub fn trim_right(s: &str) -> &str {
    s.trim_end()
}

/// Replace all occurrences of `from` with `to` in `source`.
///
/// # Examples
///
/// ```
/// use usd_tf::string_utils::replace;
///
/// assert_eq!(replace("hello world", "world", "rust"), "hello rust");
/// assert_eq!(replace("aaa", "a", "bb"), "bbbbbb");
/// ```
#[inline]
#[must_use]
pub fn replace(source: &str, from: &str, to: &str) -> String {
    if from.is_empty() || from == to {
        return source.to_string();
    }

    let mut result = source.to_string();
    let mut pos = 0usize;
    while let Some(found) = result[pos..].find(from) {
        let absolute = pos + found;
        result.replace_range(absolute..absolute + from.len(), to);
        pos = absolute + to.len();
    }
    result
}

/// Join strings with a separator.
///
/// # Examples
///
/// ```
/// use usd_tf::string_utils::join;
///
/// let parts = ["a", "b", "c"];
/// assert_eq!(join(&parts, ", "), "a, b, c");
/// assert_eq!(join(&["single"], ", "), "single");
/// assert_eq!(join::<&str>(&[], ", "), "");
/// ```
#[must_use]
pub fn join<S: AsRef<str>>(strings: &[S], separator: &str) -> String {
    strings
        .iter()
        .map(|s| s.as_ref())
        .collect::<Vec<_>>()
        .join(separator)
}

/// Split a string by a separator.
///
/// Similar to Python's str.split(). Empty strings are included.
///
/// # Examples
///
/// ```
/// use usd_tf::string_utils::split;
///
/// assert_eq!(split("a,b,c", ","), vec!["a", "b", "c"]);
/// assert_eq!(split("a,,b", ","), vec!["a", "", "b"]);
/// ```
#[must_use]
pub fn split<'a>(source: &'a str, separator: &str) -> Vec<&'a str> {
    source.split(separator).collect()
}

/// Tokenize a string by delimiter characters.
///
/// Unlike `split`, this treats consecutive delimiters as one and
/// does not return empty strings.
///
/// # Examples
///
/// ```
/// use usd_tf::string_utils::tokenize;
///
/// assert_eq!(tokenize("a b  c", " "), vec!["a", "b", "c"]);
/// assert_eq!(tokenize("  a  b  ", " "), vec!["a", "b"]);
/// assert_eq!(tokenize("a\tb\nc", " \t\n"), vec!["a", "b", "c"]);
/// ```
#[must_use]
pub fn tokenize<'a>(source: &'a str, delimiters: &str) -> Vec<&'a str> {
    let delim_set: HashSet<char> = delimiters.chars().collect();
    source
        .split(|c| delim_set.contains(&c))
        .filter(|s| !s.is_empty())
        .collect()
}

/// Tokenize a string and return as a set.
///
/// # Examples
///
/// ```
/// use usd_tf::string_utils::tokenize_to_set;
///
/// let result = tokenize_to_set("a b a c b", " ");
/// assert!(result.contains("a"));
/// assert!(result.contains("b"));
/// assert!(result.contains("c"));
/// assert_eq!(result.len(), 3);
/// ```
#[must_use]
pub fn tokenize_to_set(source: &str, delimiters: &str) -> HashSet<String> {
    tokenize(source, delimiters)
        .into_iter()
        .map(String::from)
        .collect()
}

/// Get the common prefix of two strings.
///
/// # Examples
///
/// ```
/// use usd_tf::string_utils::common_prefix;
///
/// assert_eq!(common_prefix("hello", "help"), "hel");
/// assert_eq!(common_prefix("abc", "xyz"), "");
/// assert_eq!(common_prefix("same", "same"), "same");
/// ```
#[must_use]
pub fn common_prefix<'a>(a: &'a str, b: &str) -> &'a str {
    let len = a
        .chars()
        .zip(b.chars())
        .take_while(|(ca, cb)| ca == cb)
        .count();

    // Handle multi-byte UTF-8 correctly
    let byte_len = a.chars().take(len).map(|c| c.len_utf8()).sum();
    &a[..byte_len]
}

/// Get the suffix (extension) of a string after the last delimiter.
///
/// Default delimiter is '.'.
///
/// # Examples
///
/// ```
/// use usd_tf::string_utils::get_suffix;
///
/// assert_eq!(get_suffix("file.txt", '.'), "txt");
/// assert_eq!(get_suffix("archive.tar.gz", '.'), "gz");
/// assert_eq!(get_suffix("noext", '.'), "");
/// ```
#[must_use]
pub fn get_suffix(name: &str, delimiter: char) -> &str {
    match name.rfind(delimiter) {
        Some(pos) => &name[pos + 1..],
        None => "",
    }
}

/// Get everything before the suffix.
///
/// # Examples
///
/// ```
/// use usd_tf::string_utils::get_before_suffix;
///
/// assert_eq!(get_before_suffix("file.txt", '.'), "file");
/// assert_eq!(get_before_suffix("archive.tar.gz", '.'), "archive.tar");
/// assert_eq!(get_before_suffix("noext", '.'), "noext");
/// ```
#[must_use]
pub fn get_before_suffix(name: &str, delimiter: char) -> &str {
    match name.rfind(delimiter) {
        Some(pos) => &name[..pos],
        None => name,
    }
}

/// Get the base name of a path (last component).
///
/// # Examples
///
/// ```
/// use usd_tf::string_utils::get_base_name;
///
/// assert_eq!(get_base_name("/path/to/file.txt"), "file.txt");
/// assert_eq!(get_base_name("file.txt"), "file.txt");
/// assert_eq!(get_base_name("/path/to/dir/"), "dir");
/// ```
#[must_use]
pub fn get_base_name(path: &str) -> &str {
    // Trim trailing slashes
    let path = path.trim_end_matches(['/', '\\']);

    // Find last separator
    match path.rfind(['/', '\\']) {
        Some(pos) => &path[pos + 1..],
        None => path,
    }
}

/// Get the directory name of a path.
///
/// # Examples
///
/// ```
/// use usd_tf::string_utils::get_path_name;
///
/// assert_eq!(get_path_name("/path/to/file.txt"), "/path/to/");
/// assert_eq!(get_path_name("file.txt"), "");
/// assert_eq!(get_path_name("/root"), "/");
/// ```
#[must_use]
pub fn get_path_name(path: &str) -> &str {
    match path.rfind(['/', '\\']) {
        Some(pos) => &path[..=pos],
        None => "",
    }
}

/// Parse a string to a double with lenient parsing.
///
/// Similar to atof() but safer. Returns 0.0 for invalid input.
///
/// # Examples
///
/// ```
/// use usd_tf::string_utils::string_to_double;
///
/// assert_eq!(string_to_double("3.14"), 3.14);
/// assert_eq!(string_to_double("-2.5e10"), -2.5e10);
/// assert_eq!(string_to_double("invalid"), 0.0);
/// ```
#[must_use]
pub fn string_to_double(s: &str) -> f64 {
    s.trim().parse().unwrap_or(0.0)
}

/// Parse a string to a long integer.
///
/// # Examples
///
/// ```
/// use usd_tf::string_utils::string_to_long;
///
/// assert_eq!(string_to_long("123"), Some(123));
/// assert_eq!(string_to_long("-456"), Some(-456));
/// assert_eq!(string_to_long("invalid"), None);
/// ```
#[must_use]
pub fn string_to_long(s: &str) -> Option<i64> {
    s.trim().parse().ok()
}

/// Convert a sequence of digits in `txt` to a long int value.
///
/// Matches C++ `TfStringToLong(const std::string &txt, bool *outOfRange=NULL)`.
/// If `out_of_range` is Some, it will be set to true if the value is out of range.
/// Returns i64::MIN or i64::MAX if out of range.
///
/// # Examples
///
/// ```
/// use usd_tf::string_utils::string_to_long_with_range;
///
/// let mut out_of_range = false;
/// let val = string_to_long_with_range("123", Some(&mut out_of_range));
/// assert_eq!(val, 123);
/// assert!(!out_of_range);
/// ```
#[must_use]
pub fn string_to_long_with_range(txt: &str, out_of_range: Option<&mut bool>) -> i64 {
    match txt.trim().parse::<i64>() {
        Ok(val) => {
            if let Some(flag) = out_of_range {
                *flag = false;
            }
            val
        }
        Err(_) => {
            if let Some(flag) = out_of_range {
                *flag = true;
            }
            // Try to determine if it's too large or too small
            if txt.trim().starts_with('-') {
                i64::MIN
            } else {
                i64::MAX
            }
        }
    }
}

/// Parse a string to an unsigned long integer.
///
/// # Examples
///
/// ```
/// use usd_tf::string_utils::string_to_ulong;
///
/// assert_eq!(string_to_ulong("123"), Some(123));
/// assert_eq!(string_to_ulong("-1"), None);
/// ```
#[must_use]
pub fn string_to_ulong_with_range(txt: &str, out_of_range: Option<&mut bool>) -> u64 {
    match txt.trim().parse::<u64>() {
        Ok(val) => {
            if let Some(flag) = out_of_range {
                *flag = false;
            }
            val
        }
        Err(_) => {
            if let Some(flag) = out_of_range {
                *flag = true;
            }
            u64::MAX
        }
    }
}

/// Parse a string to an unsigned long integer.
///
/// # Examples
///
/// ```
/// use usd_tf::string_utils::string_to_ulong;
///
/// assert_eq!(string_to_ulong("123"), Some(123));
/// assert_eq!(string_to_ulong("-1"), None);
/// ```
#[must_use]
pub fn string_to_ulong(s: &str) -> Option<u64> {
    s.trim().parse().ok()
}

/// Parse a string to a 64-bit signed integer.
///
/// # Arguments
///
/// * `s` - The string to parse
///
/// # Returns
///
/// A tuple of (value, out_of_range). If parsing fails, returns (0, false).
///
/// # Examples
///
/// ```
/// use usd_tf::string_utils::string_to_i64;
///
/// assert_eq!(string_to_i64("123"), (123, false));
/// assert_eq!(string_to_i64("-456"), (-456, false));
/// ```
#[must_use]
pub fn string_to_i64(s: &str) -> (i64, bool) {
    match s.trim().parse::<i64>() {
        Ok(v) => (v, false),
        Err(_) => {
            // Check if out of range
            if s.trim().starts_with('-') {
                (i64::MIN, true)
            } else {
                (i64::MAX, true)
            }
        }
    }
}

/// Parse a string to a 64-bit unsigned integer.
///
/// # Arguments
///
/// * `s` - The string to parse
///
/// # Returns
///
/// A tuple of (value, out_of_range). If parsing fails, returns (0, false).
///
/// # Examples
///
/// ```
/// use usd_tf::string_utils::string_to_u64;
///
/// assert_eq!(string_to_u64("123"), (123, false));
/// assert_eq!(string_to_u64("18446744073709551615"), (u64::MAX, false));
/// ```
#[must_use]
pub fn string_to_u64(s: &str) -> (u64, bool) {
    match s.trim().parse::<u64>() {
        Ok(v) => (v, false),
        Err(_) => (u64::MAX, true),
    }
}

/// Convert an integer to a string.
///
/// # Examples
///
/// ```
/// use usd_tf::string_utils::int_to_string;
///
/// assert_eq!(int_to_string(42), "42");
/// assert_eq!(int_to_string(-123), "-123");
/// ```
#[inline]
#[must_use]
pub fn int_to_string(i: i64) -> String {
    i.to_string()
}

/// Safely create a string from an Option<&str>.
///
/// Returns empty string if None.
///
/// # Examples
///
/// ```
/// use usd_tf::string_utils::safe_string;
///
/// assert_eq!(safe_string(Some("hello")), "hello");
/// assert_eq!(safe_string(None), "");
/// ```
#[inline]
#[must_use]
pub fn safe_string(s: Option<&str>) -> String {
    s.unwrap_or("").to_string()
}

/// Dictionary-order comparison for strings.
///
/// Matches C++ TfDictionaryLessThan:
/// - Case-insensitive character-by-character comparison
/// - Numeric substrings compared as integers (natural sort: "file2" < "file10")
/// - Underscore `_` sorts after all ASCII letters (C++ `(ch+5)&31` trick)
/// - On case-insensitive tie, original case is used as tiebreaker
///
/// # Examples
///
/// ```
/// use usd_tf::string_utils::dictionary_less_than;
///
/// assert!(dictionary_less_than("abc", "abd"));
/// assert!(dictionary_less_than("file2", "file10"));
/// // Case-insensitive: "abc" and "ABC" are equal, tiebreaker by case
/// assert!(dictionary_less_than("ABC", "abc")); // uppercase < lowercase in tiebreaker
/// ```
#[must_use]
pub fn dictionary_less_than(lhs: &str, rhs: &str) -> bool {
    fn is_digit(ch: u8) -> bool {
        ch.is_ascii_digit()
    }

    fn is_alpha(ch: u8) -> bool {
        ch.is_ascii_alphabetic()
    }

    fn mismatch(
        lhs: &[u8],
        rhs: &[u8],
        lstart: usize,
        rstart: usize,
        len: usize,
    ) -> (usize, usize) {
        for i in 0..len {
            if lhs[lstart + i] != rhs[rstart + i] {
                return (lstart + i, rstart + i);
            }
        }
        (lstart + len, rstart + len)
    }

    let lbytes = lhs.as_bytes();
    let rbytes = rhs.as_bytes();
    let lend = lbytes.len();
    let rend = rbytes.len();

    let mut lcur = 0usize;
    let mut rcur = 0usize;
    let mut cur_end = std::cmp::min(lend, rend);

    (lcur, rcur) = mismatch(lbytes, rbytes, lcur, rcur, cur_end);
    if lcur == cur_end && lend == rend {
        return false;
    }

    loop {
        if lcur == cur_end {
            break;
        }

        let l = lbytes[lcur];
        let r = rbytes[rcur];

        let both_ascii = l < 0x80 && r < 0x80;
        let differs_ignoring_case = (l & !0x20) != (r & !0x20);
        let in_letter_zone = l >= 0x40 && r >= 0x40;
        if both_ascii && differs_ignoring_case && in_letter_zone {
            return ((l + 5) & 31) < ((r + 5) & 31);
        } else if is_digit(l) || is_digit(r) {
            if is_digit(l) && is_digit(r) {
                let mut l_dig_start = lcur;
                let mut r_dig_start = rcur;
                let mut l_dig_end = lcur;
                let mut r_dig_end = rcur;

                while l_dig_start > 0 && is_digit(lbytes[l_dig_start - 1]) {
                    l_dig_start -= 1;
                }
                while r_dig_start > 0 && is_digit(rbytes[r_dig_start - 1]) {
                    r_dig_start -= 1;
                }
                while l_dig_end < lend && is_digit(lbytes[l_dig_end]) {
                    l_dig_end += 1;
                }
                while r_dig_end < rend && is_digit(rbytes[r_dig_end]) {
                    r_dig_end += 1;
                }

                while l_dig_start < l_dig_end && lbytes[l_dig_start] == b'0' {
                    l_dig_start += 1;
                }
                while r_dig_start < r_dig_end && rbytes[r_dig_start] == b'0' {
                    r_dig_start += 1;
                }

                while l_dig_start < l_dig_end
                    && r_dig_start < r_dig_end
                    && lbytes[l_dig_start] == rbytes[r_dig_start]
                {
                    l_dig_start += 1;
                    r_dig_start += 1;
                }

                if (l_dig_start == l_dig_end) ^ (r_dig_start == r_dig_end) {
                    return l_dig_start == l_dig_end && r_dig_start != r_dig_end;
                }
                if l_dig_start < l_dig_end && r_dig_start < r_dig_end {
                    let digits_l = l_dig_end - l_dig_start;
                    let digits_r = r_dig_end - r_dig_start;
                    return (digits_l, lbytes[l_dig_start]) < (digits_r, rbytes[r_dig_start]);
                }

                lcur = l_dig_end;
                rcur = r_dig_end;
                cur_end = lcur + std::cmp::min(lend - lcur, rend - rcur);
            } else {
                if lcur == 0 {
                    return l < r;
                }
                let prev = lbytes[lcur - 1];
                return if is_digit(prev) {
                    is_digit(r)
                } else {
                    is_digit(l)
                };
            }
        } else if !is_alpha(l) || !is_alpha(r) {
            return l < r;
        } else {
            lcur += 1;
            rcur += 1;
        }

        let span = cur_end.saturating_sub(lcur);
        (lcur, rcur) = mismatch(lbytes, rbytes, lcur, rcur, span);
    }

    if lcur != lend || rcur != rend {
        return lcur == lend;
    }

    let (li, ri) = mismatch(lbytes, rbytes, 0, 0, std::cmp::min(lend, rend));
    let l = lbytes[li];
    let r = rbytes[ri];
    r == b'0' || (l != b'0' && l < r)
}

/// Comparator struct for dictionary ordering.
///
/// Can be used with sorting functions.
#[derive(Debug, Clone, Copy, Default)]
pub struct DictionaryLessThan;

impl DictionaryLessThan {
    /// Compare two strings in dictionary order.
    #[inline]
    #[must_use]
    pub fn less(&self, lhs: &str, rhs: &str) -> bool {
        dictionary_less_than(lhs, rhs)
    }
}

/// Converts a glob pattern to a regex pattern.
///
/// This converts shell-style glob patterns to equivalent regular expressions:
/// - `*` becomes `.*` (match any characters)
/// - `?` becomes `.` (match single character)
/// - Special regex characters are escaped
///
/// # Examples
///
/// ```
/// use usd_tf::string_utils::glob_to_regex;
///
/// assert_eq!(glob_to_regex("*.txt"), ".*\\.txt");
/// assert_eq!(glob_to_regex("file?.dat"), "file.\\.dat");
/// ```
#[must_use]
pub fn glob_to_regex(glob: &str) -> String {
    let mut result = String::with_capacity(glob.len() * 2);

    for c in glob.chars() {
        match c {
            '*' => result.push_str(".*"),
            '?' => result.push('.'),
            // Escape regex metacharacters
            '.' | '+' | '(' | ')' | '[' | ']' | '{' | '}' | '^' | '$' | '|' | '\\' => {
                result.push('\\');
                result.push(c);
            }
            c => result.push(c),
        }
    }

    result
}

/// Unescapes backslash escape sequences in a string (matches C++ TfEscapeString).
///
/// Converts escape sequences like `\n` (backslash + 'n') into the actual byte
/// they represent. Supports: `\n \t \r \\ \" \' \a \b \f \v \0 \xHH \OOO`.
/// Any other `\X` sequence strips the backslash and keeps `X`.
///
/// # Examples
///
/// ```
/// use usd_tf::string_utils::escape_string;
///
/// assert_eq!(escape_string("hello\\nworld"), "hello\nworld");
/// assert_eq!(escape_string("path\\\\to"), "path\\to");
/// assert_eq!(escape_string("\\x41"), "A");
/// ```
#[must_use]
pub fn escape_string(s: &str) -> String {
    fn is_octal_digit(c: u8) -> bool {
        (b'0'..=b'7').contains(&c)
    }

    fn hex_to_decimal(c: u8) -> u8 {
        match c {
            b'a'..=b'f' => (c - b'a') + 10,
            b'A'..=b'F' => (c - b'A') + 10,
            _ => c - b'0',
        }
    }

    let bytes = s.as_bytes();
    let mut result = String::with_capacity(s.len());
    let mut i = 0usize;
    while i < bytes.len() {
        if bytes[i] != b'\\' {
            result.push(bytes[i] as char);
            i += 1;
            continue;
        }

        i += 1;
        if i >= bytes.len() {
            break;
        }

        match bytes[i] {
            b'\\' => result.push('\\'),
            b'a' => result.push('\x07'),
            b'b' => result.push('\x08'),
            b'f' => result.push('\x0C'),
            b'n' => result.push('\n'),
            b'r' => result.push('\r'),
            b't' => result.push('\t'),
            b'v' => result.push('\x0B'),
            b'x' => {
                let mut n: u8 = 0;
                let mut consumed = 0usize;
                while i + 1 < bytes.len()
                    && consumed != 2
                    && (bytes[i + 1] as char).is_ascii_hexdigit()
                {
                    i += 1;
                    n = n
                        .saturating_mul(16)
                        .saturating_add(hex_to_decimal(bytes[i]));
                    consumed += 1;
                }
                result.push(n as char);
            }
            b'0'..=b'7' => {
                i -= 1;
                let mut n: u8 = 0;
                let mut consumed = 0usize;
                while i + 1 < bytes.len() && consumed != 3 && is_octal_digit(bytes[i + 1]) {
                    i += 1;
                    n = n
                        .saturating_mul(8)
                        .saturating_add(bytes[i].saturating_sub(b'0'));
                    consumed += 1;
                }
                result.push(n as char);
            }
            other => result.push(other as char),
        }
        i += 1;
    }

    result
}

/// Escapes a string for inclusion in XML.
///
/// Replaces special XML characters with their entity references:
/// - `&` -> `&amp;`
/// - `<` -> `&lt;`
/// - `>` -> `&gt;`
/// - `"` -> `&quot;`
/// - `'` -> `&apos;`
///
/// # Examples
///
/// ```
/// use usd_tf::string_utils::get_xml_escaped_string;
///
/// assert_eq!(get_xml_escaped_string("<tag>"), "&lt;tag&gt;");
/// assert_eq!(get_xml_escaped_string("a & b"), "a &amp; b");
/// ```
#[must_use]
pub fn get_xml_escaped_string(s: &str) -> String {
    let mut result = String::with_capacity(s.len());

    for c in s.chars() {
        match c {
            '&' => result.push_str("&amp;"),
            '<' => result.push_str("&lt;"),
            '>' => result.push_str("&gt;"),
            '"' => result.push_str("&quot;"),
            '\'' => result.push_str("&apos;"),
            c => result.push(c),
        }
    }

    result
}

/// Concatenates two path components.
///
/// Handles trailing/leading slashes appropriately.
///
/// # Examples
///
/// ```
/// use usd_tf::string_utils::cat_paths;
///
/// assert_eq!(cat_paths("/usr", "local"), "/usr/local");
/// assert_eq!(cat_paths("/usr/", "local"), "/usr/local");
/// assert_eq!(cat_paths("/usr", "/local"), "/usr/local");
/// ```
#[must_use]
pub fn cat_paths(prefix: &str, suffix: &str) -> String {
    // C++ TfStringCatPaths: TfNormPath(prefix + "/" + suffix).
    // Empty prefix case: TfNormPath("/" + suffix) — prepends "/".
    if prefix.is_empty() {
        return crate::path_utils::norm_path(&format!("/{}", suffix));
    }
    crate::path_utils::norm_path(&format!("{}/{}", prefix, suffix))
}

/// Converts a string to a bool.
///
/// Accepts: "true", "yes", "on", "1" as true (case-insensitive)
/// All other values return false.
///
/// # Examples
///
/// ```
/// use usd_tf::string_utils::unstringify_bool;
///
/// assert_eq!(unstringify_bool("true"), true);
/// assert_eq!(unstringify_bool("YES"), true);
/// assert_eq!(unstringify_bool("1"), true);
/// assert_eq!(unstringify_bool("false"), false);
/// assert_eq!(unstringify_bool("anything"), false);
/// ```
#[must_use]
pub fn unstringify_bool(s: &str) -> bool {
    let lower = s.to_lowercase();
    matches!(lower.as_str(), "true" | "yes" | "on" | "1")
}

/// Stringify a bool value.
///
/// # Examples
///
/// ```
/// use usd_tf::string_utils::stringify_bool;
///
/// assert_eq!(stringify_bool(true), "true");
/// assert_eq!(stringify_bool(false), "false");
/// ```
#[must_use]
pub fn stringify_bool(v: bool) -> &'static str {
    if v { "true" } else { "false" }
}

/// Stringify a float value with appropriate precision.
///
/// Uses minimal-roundtrip representation matching C++ `pxr_double_conversion`
/// `ToShortestSingle` behaviour.
///
/// # Examples
///
/// ```
/// use usd_tf::string_utils::stringify_float;
///
/// assert_eq!(stringify_float(3.14), "3.14");
/// assert_eq!(stringify_float(1.0), "1");
/// ```
#[must_use]
pub fn stringify_float(v: f32) -> String {
    // Fast path for small whole numbers that fit in i64 without precision loss.
    if v.fract() == 0.0 && v.abs() < 1e10 {
        return format!("{}", v as i64);
    }
    // Rust's Display for f32 already produces the shortest roundtrip representation.
    format!("{}", v)
}

/// Stringify a double value with appropriate precision.
///
/// Uses minimal-roundtrip representation matching C++ `pxr_double_conversion`
/// `ToShortest` behaviour. Large integers like `1e15` are rendered as `"1e15"`.
///
/// # Examples
///
/// ```
/// use usd_tf::string_utils::stringify_double;
///
/// assert_eq!(stringify_double(3.14159265358979), "3.14159265358979");
/// assert_eq!(stringify_double(1.0), "1");
/// ```
#[must_use]
pub fn stringify_double(v: f64) -> String {
    // Fast path for small whole numbers that fit in i64 without precision loss.
    // The 1e10 bound keeps us well away from the f64->i64 overflow boundary and
    // matches the range where decimal notation is clearly more compact.
    if v.fract() == 0.0 && v.abs() < 1e10 {
        return format!("{}", v as i64);
    }
    // Rust's Display for f64 already produces the shortest roundtrip representation.
    format!("{}", v)
}

/// Check if a string is a valid C/Python identifier.
///
/// An identifier is valid if it:
/// - Is at least one character long
/// - Starts with a letter or underscore
/// - Contains only letters, underscores, and digits
///
/// # Examples
///
/// ```
/// use usd_tf::string_utils::is_valid_identifier;
///
/// assert!(is_valid_identifier("hello"));
/// assert!(is_valid_identifier("_private"));
/// assert!(is_valid_identifier("var123"));
/// assert!(!is_valid_identifier("123var"));
/// assert!(!is_valid_identifier(""));
/// assert!(!is_valid_identifier("has-dash"));
/// ```
#[must_use]
pub fn is_valid_identifier(s: &str) -> bool {
    let mut chars = s.chars();

    match chars.next() {
        None => false, // Empty string
        Some(first) => {
            // First char must be letter or underscore
            if !first.is_ascii_alphabetic() && first != '_' {
                return false;
            }
            // Rest must be letter, digit, or underscore
            chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
        }
    }
}

/// Make a valid identifier from input string.
///
/// Replaces invalid characters with underscores.
/// If the string is empty, returns "_".
/// If it starts with a digit, prepends "_".
///
/// # Examples
///
/// ```
/// use usd_tf::string_utils::make_valid_identifier;
///
/// assert_eq!(make_valid_identifier("hello"), "hello");
/// assert_eq!(make_valid_identifier("has-dash"), "has_dash");
/// assert_eq!(make_valid_identifier("123"), "_123");
/// assert_eq!(make_valid_identifier(""), "_");
/// ```
#[must_use]
pub fn make_valid_identifier(s: &str) -> String {
    if s.is_empty() {
        return "_".to_string();
    }

    let mut result = String::with_capacity(s.len() + 1);
    let mut chars = s.chars().peekable();

    // Handle first character
    if let Some(&first) = chars.peek() {
        if first.is_ascii_digit() {
            result.push('_');
        }
    }

    for c in chars {
        if c.is_ascii_alphanumeric() || c == '_' {
            result.push(c);
        } else {
            result.push('_');
        }
    }

    result
}

/// Tokenize a string with support for quoted substrings.
///
/// Tokens delimited by `"`, `'`, or `` ` `` are treated as single tokens with
/// the enclosing quote characters stripped, matching C++ `TfQuotedStringTokenize`.
/// Backslash escapes inside quoted regions are recognised (the escaped character
/// is included verbatim in the output token).
///
/// # Examples
///
/// ```
/// use usd_tf::string_utils::quoted_string_tokenize;
///
/// let result = quoted_string_tokenize(r#"hello "quoted string" world"#, " ");
/// assert_eq!(result, Ok(vec!["hello".to_string(), "quoted string".to_string(), "world".to_string()]));
/// ```
#[must_use]
pub fn quoted_string_tokenize(source: &str, delimiters: &str) -> Result<Vec<String>, String> {
    let quotes = ['"', '\'', '`'];
    if delimiters.chars().any(|c| quotes.contains(&c)) {
        return Err("Cannot use quotes as delimiters.".to_string());
    }

    fn find_first_not_of(source: &str, delimiters: &str, offset: usize) -> Option<usize> {
        if delimiters.is_empty() {
            return (offset < source.len()).then_some(offset);
        }
        source[offset..]
            .char_indices()
            .find(|(_, c)| !delimiters.contains(*c))
            .map(|(i, _)| offset + i)
    }

    fn find_first_of(source: &str, chars: &[char], offset: usize) -> Option<usize> {
        source[offset..]
            .char_indices()
            .find(|(_, c)| chars.contains(c))
            .map(|(i, _)| offset + i)
    }

    fn find_first_of_not_escaped(source: &str, chars: &[char], offset: usize) -> Option<usize> {
        let mut pos = find_first_of(source, chars, offset);
        while let Some(idx) = pos {
            if idx == 0 || source.as_bytes()[idx - 1] != b'\\' {
                return Some(idx);
            }
            pos = find_first_of(source, chars, idx + 1);
        }
        None
    }

    let mut result = Vec::new();
    let mut i = 0usize;
    while let Some(start) = find_first_not_of(source, delimiters, i) {
        i = start;
        let mut token = String::new();

        loop {
            let quote_index = find_first_of_not_escaped(source, &quotes, i).unwrap_or(usize::MAX);
            let delim_index = if delimiters.is_empty() {
                usize::MAX
            } else {
                source[i..]
                    .char_indices()
                    .find(|(_, c)| delimiters.contains(*c))
                    .map(|(off, _)| i + off)
                    .unwrap_or(usize::MAX)
            };

            if quote_index >= delim_index {
                break;
            }

            if i < quote_index {
                token.push_str(&source[i..quote_index]);
            }

            let quote_char = source.as_bytes()[quote_index] as char;
            let Some(end_quote) = find_first_of_not_escaped(source, &[quote_char], quote_index + 1)
            else {
                return Err(format!(
                    "String is missing an end-quote ('{}'): {}",
                    quote_char, source
                ));
            };

            if quote_index + 1 < end_quote {
                token.push_str(&source[quote_index + 1..end_quote]);
            }
            i = end_quote + 1;
        }

        let delim_index = if delimiters.is_empty() {
            None
        } else {
            source[i..]
                .char_indices()
                .find(|(_, c)| delimiters.contains(*c))
                .map(|(off, _)| i + off)
        };

        if let Some(di) = delim_index {
            token.push_str(&source[i..di]);
            i = di + 1;
        } else {
            token.push_str(&source[i..]);
        }

        for quote in quotes {
            token = replace(&token, &format!("\\{}", quote), &quote.to_string());
        }
        result.push(token);

        if delim_index.is_none() {
            break;
        }
    }

    Ok(result)
}

/// Tokenize a string by matching delimiters.
///
/// Words begin with `open_delim` and end with matching `close_delim`.
/// Nested delimiters are preserved.
///
/// # Examples
///
/// ```
/// use usd_tf::string_utils::matched_string_tokenize;
///
/// // Basic usage: extract balanced {}-delimited tokens
/// let result = matched_string_tokenize("{a} string {to {be} split}", '{', '}', '\\');
/// assert_eq!(result, Ok(vec!["a".to_string(), "to {be} split".to_string()]));
///
/// // Escape character prevents delimiter from being treated as such
/// let result = matched_string_tokenize("(a\\(b)", '(', ')', '\\');
/// assert_eq!(result, Ok(vec!["a(b".to_string()]));
///
/// // Pass '\0' to disable escape handling entirely
/// let result = matched_string_tokenize("{a} {b}", '{', '}', '\0');
/// assert_eq!(result, Ok(vec!["a".to_string(), "b".to_string()]));
/// ```
#[must_use]
pub fn matched_string_tokenize(
    source: &str,
    open_delim: char,
    close_delim: char,
    escape: char,
) -> Result<Vec<String>, String> {
    // Escape char cannot double as a delimiter — matches C++ guard.
    if escape != '\0' && (escape == open_delim || escape == close_delim) {
        return Err("Escape character cannot be a delimiter.".to_string());
    }

    let bytes = source.as_bytes();
    let src_len = bytes.len();

    // Build set of special byte values we scan for.
    // Works correctly only when all special chars are ASCII (matches C++ assumption).
    let mut special: Vec<u8> = Vec::with_capacity(3);
    if escape != '\0' {
        special.push(escape as u8);
    }
    special.push(open_delim as u8);
    let same_delims = open_delim == close_delim;
    if !same_delims {
        special.push(close_delim as u8);
    }

    // Helper: find first byte in `special` at or after `from`.
    let find_special =
        |from: usize| -> Option<usize> { (from..src_len).find(|&i| special.contains(&bytes[i])) };

    // Check for an unescaped close delimiter appearing before any open delimiter.
    let first_close = (0..src_len).find(|&i| bytes[i] == close_delim as u8);
    if let Some(ci) = first_close {
        let unescaped = ci == 0 || bytes[ci - 1] != escape as u8;
        let first_open = (0..src_len).find(|&i| bytes[i] == open_delim as u8);
        if unescaped && first_open.map_or(true, |oi| ci < oi) {
            return Err(format!(
                "String has unmatched close delimiter ('{}', '{}'): {}",
                open_delim, close_delim, source
            ));
        }
    }

    let mut result: Vec<String> = Vec::new();
    let mut open_idx: usize = 0; // byte index of the current open delimiter

    // Outer loop: find each top-level open delimiter.
    while let Some(oi) = (open_idx..src_len).find(|&i| bytes[i] == open_delim as u8) {
        let mut open_count: usize = 1;
        let mut close_count: usize = 0;
        open_idx = oi;
        let mut next_idx = oi;
        let mut token = String::new();

        // Inner loop: walk forward until open_count == close_count.
        while close_count != open_count {
            let found = find_special(next_idx + 1);
            let Some(ni) = found else {
                return Err(format!(
                    "String has unmatched open delimiter ('{}', '{}'): {}",
                    open_delim, close_delim, source
                ));
            };

            if bytes[ni] == escape as u8 {
                // Escape: consume the char after it literally, strip the escape itself.
                let after = ni + 1;
                if after < src_len.saturating_sub(1) {
                    // Append segment from (open_idx+1) up to (not including) the escape.
                    token.push_str(&source[open_idx + 1..ni]);
                    // Append the escaped character (raw, without the backslash).
                    token.push(bytes[after] as char);
                    // Reset both indices past the escaped char; next scan starts from `after`.
                    open_idx = after;
                    next_idx = after;
                }
                // If after >= src_len-1 we just skip (edge case, same as C++).
                // `continue` so we don't overwrite next_idx with `ni` below.
                continue;
            } else if !same_delims && bytes[ni] == open_delim as u8 {
                open_count += 1;
            } else {
                // close delimiter
                close_count += 1;
            }

            next_idx = ni;
        }

        // Append the final segment between open_idx+1 and next_idx.
        if next_idx > open_idx + 1 {
            token.push_str(&source[open_idx + 1..next_idx]);
        }

        result.push(token);
        open_idx = next_idx + 1;
    }

    // Trailing unescaped close delimiter check.
    let trailing_close = (open_idx..src_len).find(|&i| bytes[i] == close_delim as u8);
    if let Some(ci) = trailing_close {
        if ci == 0 || bytes[ci - 1] != escape as u8 {
            return Err(format!(
                "String has unmatched close delimiter ('{}', '{}'): {}",
                open_delim, close_delim, source
            ));
        }
    }

    Ok(result)
}

/// Double-to-string conversion with roundtrip precision.
///
/// Writes the shortest representation that will roundtrip back to the same value.
///
/// # Examples
///
/// ```
/// use usd_tf::string_utils::double_to_string;
///
/// assert_eq!(double_to_string(3.14, true), "3.14");
/// assert_eq!(double_to_string(1.0, true), "1.0");
/// assert_eq!(double_to_string(1.0, false), "1");
/// ```
#[must_use]
/// A type which offers streaming for floats in a canonical
/// format that can safely roundtrip with the minimal number of digits.
///
/// Matches C++ `TfStreamFloat` struct.
///
/// # Examples
///
/// ```
/// use usd_tf::string_utils::StreamFloat;
/// use std::fmt;
///
/// let val = StreamFloat::new(3.14f32);
/// println!("{}", val); // Prints in canonical format
/// ```
#[derive(Debug, Clone, Copy)]
pub struct StreamFloat {
    value: f32,
}

impl StreamFloat {
    /// Creates a new `StreamFloat` wrapper.
    ///
    /// Matches C++ `TfStreamFloat(float)` constructor.
    #[inline]
    pub fn new(value: f32) -> Self {
        Self { value }
    }

    /// Returns the wrapped float value.
    #[inline]
    pub fn value(self) -> f32 {
        self.value
    }
}

impl std::fmt::Display for StreamFloat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Use Rust's built-in formatting which provides canonical representation
        // This matches C++ behavior of using double_conversion for shortest representation
        write!(f, "{}", self.value)
    }
}

/// A type which offers streaming for doubles in a canonical
/// format that can safely roundtrip with the minimal number of digits.
///
/// Matches C++ `TfStreamDouble` struct.
///
/// # Examples
///
/// ```
/// use usd_tf::string_utils::StreamDouble;
/// use std::fmt;
///
/// let val = StreamDouble::new(3.14159f64);
/// println!("{}", val); // Prints in canonical format
/// ```
#[derive(Debug, Clone, Copy)]
pub struct StreamDouble {
    value: f64,
}

impl StreamDouble {
    /// Creates a new `StreamDouble` wrapper.
    ///
    /// Matches C++ `TfStreamDouble(double)` constructor.
    #[inline]
    pub fn new(value: f64) -> Self {
        Self { value }
    }

    /// Returns the wrapped double value.
    #[inline]
    pub fn value(self) -> f64 {
        self.value
    }
}

impl std::fmt::Display for StreamDouble {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Use Rust's built-in formatting which provides canonical representation
        // This matches C++ behavior of using double_conversion for shortest representation
        write!(f, "{}", self.value)
    }
}

/// Returns a string formed by a printf()-like specification.
///
/// `string_printf` is a memory-safe way of forming a string using
/// printf()-like formatting. This is a wrapper that uses Rust's `format!` macro.
///
/// # Examples
///
/// ```ignore
/// use usd_tf::string_printf;
///
/// let msg = string_printf!("Hello, {}!", "world");
/// assert_eq!(msg, "Hello, world!");
/// ```
#[macro_export]
macro_rules! string_printf {
    ($($arg:tt)*) => {
        format!($($arg)*)
    };
}

/// Returns a string formed by a printf()-like specification (function form).
///
/// This is equivalent to `string_printf!` macro but as a function.
/// Uses Rust's `format!` macro internally.
///
/// # Examples
///
/// ```ignore
/// use usd_tf::string_printf_fn;
///
/// // Note: This is a simple wrapper for API compatibility
/// let msg = string_printf_fn("Hello, world!");
/// ```
pub fn string_printf_fn(fmt: &str) -> String {
    // In Rust, we use format! macro directly
    // This function is provided for API compatibility
    fmt.to_string()
}

/// Safely create a String from a (possibly NULL) char*.
///
/// If `ptr` is None or empty, the empty string is safely returned.
/// This matches C++ `TfSafeString(const char* ptr)`.
///
/// # Examples
///
/// ```
/// use usd_tf::string_utils::safe_string_from_ptr;
///
/// assert_eq!(safe_string_from_ptr(Some("hello")), "hello");
/// assert_eq!(safe_string_from_ptr(None), "");
/// ```
#[inline]
#[must_use]
pub fn safe_string_from_ptr(ptr: Option<&str>) -> String {
    ptr.unwrap_or("").to_string()
}

/// Returns the given integer as a string.
///
/// This matches C++ `TfIntToString(int i)`.
///
/// # Examples
///
/// ```
/// use usd_tf::string_utils::int_to_string;
///
/// assert_eq!(int_to_string(42), "42");
/// assert_eq!(int_to_string(-10), "-10");
/// ```
#[inline]
#[must_use]
pub fn int_to_string_i32(i: i32) -> String {
    i.to_string()
}

/// Parse a string to a 64-bit signed integer with out-of-range flag.
///
/// This matches C++ `TfStringToInt64(const std::string &txt, bool *outOfRange)`.
///
/// # Arguments
///
/// * `txt` - The string to parse
/// * `out_of_range` - Optional mutable bool to set if value is out of range
///
/// # Returns
///
/// The parsed value, or i64::MIN/i64::MAX if out of range.
///
/// # Examples
///
/// ```
/// use usd_tf::string_utils::string_to_i64_with_range;
///
/// let mut out_of_range = false;
/// assert_eq!(string_to_i64_with_range("123", Some(&mut out_of_range)), 123);
/// assert!(!out_of_range);
///
/// let mut out_of_range = false;
/// string_to_i64_with_range("999999999999999999999", Some(&mut out_of_range));
/// assert!(out_of_range);
/// ```
#[must_use]
pub fn string_to_i64_with_range(txt: &str, out_of_range: Option<&mut bool>) -> i64 {
    match txt.trim().parse::<i64>() {
        Ok(val) => {
            if let Some(flag) = out_of_range {
                *flag = false;
            }
            val
        }
        Err(_) => {
            if let Some(flag) = out_of_range {
                *flag = true;
            }
            // Try to determine if it's too large or too small
            if txt.trim().starts_with('-') {
                i64::MIN
            } else {
                i64::MAX
            }
        }
    }
}

/// Parse a string to a 64-bit unsigned integer with out-of-range flag.
///
/// This matches C++ `TfStringToUInt64(const std::string &txt, bool *outOfRange)`.
///
/// # Arguments
///
/// * `txt` - The string to parse
/// * `out_of_range` - Optional mutable bool to set if value is out of range
///
/// # Returns
///
/// The parsed value, or u64::MAX if out of range.
///
/// # Examples
///
/// ```
/// use usd_tf::string_utils::string_to_u64_with_range;
///
/// let mut out_of_range = false;
/// assert_eq!(string_to_u64_with_range("123", Some(&mut out_of_range)), 123);
/// assert!(!out_of_range);
///
/// let mut out_of_range = false;
/// string_to_u64_with_range("999999999999999999999", Some(&mut out_of_range));
/// assert!(out_of_range);
/// ```
#[must_use]
pub fn string_to_u64_with_range(txt: &str, out_of_range: Option<&mut bool>) -> u64 {
    match txt.trim().parse::<u64>() {
        Ok(val) => {
            if let Some(flag) = out_of_range {
                *flag = false;
            }
            val
        }
        Err(_) => {
            if let Some(flag) = out_of_range {
                *flag = true;
            }
            u64::MAX
        }
    }
}

/// Convert a double to a string representation.
///
/// This matches C++ `TfDoubleToString(double d, char* buffer, int len, bool emitTrailingZero)`.
/// In Rust, we return a String instead of writing to a buffer.
///
/// # Arguments
///
/// * `d` - The double value to convert
/// * `emit_trailing_zero` - If true, emit trailing ".0" for integer values
///
/// # Returns
///
/// String representation of the double.
///
/// # Examples
///
/// ```
/// use usd_tf::string_utils::double_to_string;
///
/// assert_eq!(double_to_string(123.0, true), "123.0");
/// assert_eq!(double_to_string(123.0, false), "123");
/// assert_eq!(double_to_string(123.45, false), "123.45");
/// ```
#[must_use]
pub fn double_to_string(d: f64, emit_trailing_zero: bool) -> String {
    if d.is_nan() {
        return "nan".to_string();
    }
    if d.is_infinite() {
        return if d.is_sign_positive() { "inf" } else { "-inf" }.to_string();
    }

    let abs = d.abs();
    let s = if abs != 0.0 && !(1e-6..1e15).contains(&abs) {
        let raw = format!("{:.16e}", d);
        let e_pos = raw
            .find('e')
            .expect("scientific notation must contain exponent");
        let mut mantissa = raw[..e_pos].to_string();
        while mantissa.ends_with('0') {
            mantissa.pop();
        }
        if mantissa.ends_with('.') {
            if emit_trailing_zero {
                mantissa.push('0');
            } else {
                mantissa.pop();
            }
        }
        format!("{mantissa}{}", &raw[e_pos..])
    } else {
        format!("{}", d)
    };

    if emit_trailing_zero && d.fract() == 0.0 && !s.contains('.') {
        format!("{}.0", s)
    } else {
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_starts_with() {
        assert!(starts_with("hello", "hel"));
        assert!(starts_with("hello", ""));
        assert!(!starts_with("hello", "world"));
        assert!(!starts_with("hi", "hello"));
    }

    #[test]
    fn test_ends_with() {
        assert!(ends_with("hello", "llo"));
        assert!(ends_with("hello", ""));
        assert!(!ends_with("hello", "hel"));
    }

    #[test]
    fn test_contains() {
        assert!(contains("hello world", "lo wo"));
        assert!(contains("hello", ""));
        assert!(!contains("hello", "xyz"));
    }

    #[test]
    fn test_to_lower() {
        assert_eq!(to_lower("HELLO"), "hello");
        assert_eq!(to_lower("HeLLo"), "hello");
        assert_eq!(to_lower("123"), "123");
    }

    #[test]
    fn test_to_upper() {
        assert_eq!(to_upper("hello"), "HELLO");
        assert_eq!(to_upper("HeLLo"), "HELLO");
    }

    #[test]
    fn test_to_lower_ascii() {
        assert_eq!(to_lower_ascii("HELLO"), "hello");
        assert_eq!(to_lower_ascii("HeLLo123"), "hello123");
    }

    #[test]
    fn test_capitalize() {
        assert_eq!(capitalize("hello"), "Hello");
        assert_eq!(capitalize("HELLO"), "HELLO"); // C++ only uppercases first, leaves rest
        assert_eq!(capitalize("h"), "H");
        assert_eq!(capitalize(""), "");
    }

    #[test]
    fn test_trim() {
        assert_eq!(trim("  hello  "), "hello");
        assert_eq!(trim("hello"), "hello");
        assert_eq!(trim("   "), "");
    }

    #[test]
    fn test_trim_chars() {
        assert_eq!(trim_chars("...hello...", "."), "hello");
        assert_eq!(trim_chars("xxhelloxx", "x"), "hello");
        assert_eq!(trim_chars("abchelloabc", "abc"), "hello");
    }

    #[test]
    fn test_replace() {
        assert_eq!(replace("hello world", "world", "rust"), "hello rust");
        assert_eq!(replace("aaa", "a", "bb"), "bbbbbb");
        assert_eq!(replace("hello", "x", "y"), "hello");
    }

    #[test]
    fn test_join() {
        assert_eq!(join(&["a", "b", "c"], ", "), "a, b, c");
        assert_eq!(join(&["single"], ", "), "single");
        assert_eq!(join::<&str>(&[], ", "), "");
    }

    #[test]
    fn test_split() {
        assert_eq!(split("a,b,c", ","), vec!["a", "b", "c"]);
        assert_eq!(split("a,,b", ","), vec!["a", "", "b"]);
        assert_eq!(split("abc", ","), vec!["abc"]);
    }

    #[test]
    fn test_tokenize() {
        assert_eq!(tokenize("a b  c", " "), vec!["a", "b", "c"]);
        assert_eq!(tokenize("  a  b  ", " "), vec!["a", "b"]);
        assert_eq!(tokenize("a\tb\nc", " \t\n"), vec!["a", "b", "c"]);
        assert_eq!(tokenize("", " "), Vec::<&str>::new());
    }

    #[test]
    fn test_tokenize_to_set() {
        let result = tokenize_to_set("a b a c b", " ");
        assert_eq!(result.len(), 3);
        assert!(result.contains("a"));
        assert!(result.contains("b"));
        assert!(result.contains("c"));
    }

    #[test]
    fn test_common_prefix() {
        assert_eq!(common_prefix("hello", "help"), "hel");
        assert_eq!(common_prefix("abc", "xyz"), "");
        assert_eq!(common_prefix("same", "same"), "same");
        assert_eq!(common_prefix("", "abc"), "");
    }

    #[test]
    fn test_get_suffix() {
        assert_eq!(get_suffix("file.txt", '.'), "txt");
        assert_eq!(get_suffix("archive.tar.gz", '.'), "gz");
        assert_eq!(get_suffix("noext", '.'), "");
    }

    #[test]
    fn test_get_before_suffix() {
        assert_eq!(get_before_suffix("file.txt", '.'), "file");
        assert_eq!(get_before_suffix("archive.tar.gz", '.'), "archive.tar");
        assert_eq!(get_before_suffix("noext", '.'), "noext");
    }

    #[test]
    fn test_get_base_name() {
        assert_eq!(get_base_name("/path/to/file.txt"), "file.txt");
        assert_eq!(get_base_name("file.txt"), "file.txt");
        assert_eq!(get_base_name("/path/to/dir/"), "dir");
        assert_eq!(get_base_name("/"), "");
    }

    #[test]
    fn test_get_path_name() {
        assert_eq!(get_path_name("/path/to/file.txt"), "/path/to/");
        assert_eq!(get_path_name("file.txt"), "");
        assert_eq!(get_path_name("/root"), "/");
    }

    #[test]
    fn test_string_to_double() {
        assert!((string_to_double("3.14") - 3.14).abs() < 0.001);
        assert!((string_to_double("-2.5") - (-2.5)).abs() < 0.001);
        assert_eq!(string_to_double("invalid"), 0.0);
        assert_eq!(string_to_double(""), 0.0);
    }

    #[test]
    fn test_string_to_long() {
        assert_eq!(string_to_long("123"), Some(123));
        assert_eq!(string_to_long("-456"), Some(-456));
        assert_eq!(string_to_long("invalid"), None);
    }

    #[test]
    fn test_string_to_ulong() {
        assert_eq!(string_to_ulong("123"), Some(123));
        assert_eq!(string_to_ulong("-1"), None);
    }

    #[test]
    fn test_int_to_string() {
        assert_eq!(int_to_string(42), "42");
        assert_eq!(int_to_string(-123), "-123");
        assert_eq!(int_to_string(0), "0");
    }

    #[test]
    fn test_safe_string() {
        assert_eq!(safe_string(Some("hello")), "hello");
        assert_eq!(safe_string(None), "");
    }

    #[test]
    fn test_dictionary_less_than() {
        assert!(dictionary_less_than("abc", "abd"));
        assert!(!dictionary_less_than("abd", "abc"));
        assert!(dictionary_less_than("ABC", "abd"));
        assert!(dictionary_less_than("file1", "file2"));
    }

    #[test]
    fn test_dictionary_less_than_struct() {
        let cmp = DictionaryLessThan;
        assert!(cmp.less("a", "b"));
        assert!(!cmp.less("b", "a"));
    }

    #[test]
    fn test_glob_to_regex() {
        assert_eq!(glob_to_regex("*.txt"), ".*\\.txt");
        assert_eq!(glob_to_regex("file?.dat"), "file.\\.dat");
        assert_eq!(glob_to_regex("simple"), "simple");
        assert_eq!(glob_to_regex("a*b?c"), "a.*b.c");
    }

    #[test]
    fn test_escape_string() {
        // escape_string is an UNESCAPER (matches C++ TfEscapeString)
        assert_eq!(escape_string("hello\\nworld"), "hello\nworld");
        assert_eq!(escape_string("path\\\\to"), "path\\to");
        assert_eq!(escape_string("tab\\there"), "tab\there");
        assert_eq!(escape_string("simple"), "simple");
    }

    #[test]
    fn test_get_xml_escaped_string() {
        assert_eq!(get_xml_escaped_string("<tag>"), "&lt;tag&gt;");
        assert_eq!(get_xml_escaped_string("a & b"), "a &amp; b");
        assert_eq!(get_xml_escaped_string("\"quote\""), "&quot;quote&quot;");
        assert_eq!(get_xml_escaped_string("simple"), "simple");
    }

    #[test]
    fn test_cat_paths() {
        assert_eq!(cat_paths("/usr", "local"), "/usr/local");
        assert_eq!(cat_paths("/usr/", "local"), "/usr/local");
        assert_eq!(cat_paths("/usr", "/local"), "/usr/local");
        assert_eq!(cat_paths("/usr/", "/local"), "/usr/local");
        assert_eq!(cat_paths("", "path"), "/path"); // C++ prepends / for empty prefix
        assert_eq!(cat_paths("path", ""), "path");
    }

    #[test]
    fn test_unstringify_bool() {
        assert!(unstringify_bool("true"));
        assert!(unstringify_bool("TRUE"));
        assert!(unstringify_bool("yes"));
        assert!(unstringify_bool("YES"));
        assert!(unstringify_bool("on"));
        assert!(unstringify_bool("1"));
        assert!(!unstringify_bool("false"));
        assert!(!unstringify_bool("no"));
        assert!(!unstringify_bool("0"));
        assert!(!unstringify_bool("anything"));
    }

    #[test]
    fn test_stringify_bool() {
        assert_eq!(stringify_bool(true), "true");
        assert_eq!(stringify_bool(false), "false");
    }

    #[test]
    fn test_stringify_float() {
        assert_eq!(stringify_float(1.0), "1");
        assert_eq!(stringify_float(3.14), "3.14");
    }

    #[test]
    fn test_stringify_double() {
        assert_eq!(stringify_double(1.0), "1");
        assert_eq!(stringify_double(3.14159265358979), "3.14159265358979");
    }

    #[test]
    fn test_matched_string_tokenize_basic() {
        // Basic balanced extraction, no nesting.
        let r = matched_string_tokenize("{a} string {b}", '{', '}', '\\').unwrap();
        assert_eq!(r, vec!["a", "b"]);
    }

    #[test]
    fn test_matched_string_tokenize_nested() {
        // Nested delimiters are preserved in token content.
        let r = matched_string_tokenize("{a} string {to {be} split}", '{', '}', '\\').unwrap();
        assert_eq!(r, vec!["a", "to {be} split"]);
    }

    #[test]
    fn test_matched_string_tokenize_escape_open() {
        // Escaped open delimiter is treated as literal, not counted.
        let r = matched_string_tokenize("(a\\(b)", '(', ')', '\\').unwrap();
        assert_eq!(r, vec!["a(b"]);
    }

    #[test]
    fn test_matched_string_tokenize_escape_close() {
        // Escaped close delimiter inside token does not close the bracket.
        let r = matched_string_tokenize("(a\\)b)", '(', ')', '\\').unwrap();
        assert_eq!(r, vec!["a)b"]);
    }

    #[test]
    fn test_matched_string_tokenize_no_escape() {
        // Passing '\0' disables escape processing entirely.
        let r = matched_string_tokenize("{a} {b}", '{', '}', '\0').unwrap();
        assert_eq!(r, vec!["a", "b"]);
    }

    #[test]
    fn test_matched_string_tokenize_empty() {
        // Empty source produces empty result.
        let r = matched_string_tokenize("", '{', '}', '\\').unwrap();
        assert!(r.is_empty());
    }

    #[test]
    fn test_matched_string_tokenize_no_tokens() {
        // Source with no delimiters at all.
        let r = matched_string_tokenize("hello world", '{', '}', '\\').unwrap();
        assert!(r.is_empty());
    }

    #[test]
    fn test_matched_string_tokenize_err_escape_is_delim() {
        // Escape char == open delimiter must error.
        assert!(matched_string_tokenize("(a)", '(', ')', '(').is_err());
        // Escape char == close delimiter must error.
        assert!(matched_string_tokenize("(a)", '(', ')', ')').is_err());
    }

    #[test]
    fn test_matched_string_tokenize_err_unmatched_open() {
        // Unmatched open delimiter produces Err.
        assert!(matched_string_tokenize("{open", '{', '}', '\\').is_err());
    }

    #[test]
    fn test_matched_string_tokenize_err_unmatched_close() {
        // Bare close delimiter before any open is an error.
        assert!(matched_string_tokenize("}close", '{', '}', '\\').is_err());
    }
}
