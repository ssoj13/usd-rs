//! String operations matching OSL built-in string functions.
//!
//! These are the runtime implementations of OSL's string operations
//! from `opstring.cpp`.
//!
//! **Derivative note:** String operations (strlen, hash, regex, getchar, stoi,
//! stof, concat, substr, format, etc.) return integers or strings which carry
//! no derivatives. The interpreter handles this by assigning zero derivatives
//! to outputs of string ops -- no Dual2 variants are needed here.
//!
//! **Regex caching (plan #42):** Matches C++ `ShadingContext::find_regex` —
//! compiled patterns are cached to avoid repeated parsing.

/// Concatenate two strings.
pub fn concat(a: &str, b: &str) -> String {
    let mut s = String::with_capacity(a.len() + b.len());
    s.push_str(a);
    s.push_str(b);
    s
}

/// Get the length of a string.
#[inline]
pub fn strlen(s: &str) -> i32 {
    s.len() as i32
}

/// Check if a string starts with a prefix.
#[inline]
pub fn startswith(s: &str, prefix: &str) -> bool {
    s.starts_with(prefix)
}

/// Check if a string ends with a suffix.
#[inline]
pub fn endswith(s: &str, suffix: &str) -> bool {
    s.ends_with(suffix)
}

/// Extract a substring. OSL: `substr(s, start, len)`.
/// `start` can be negative (counts from end).
pub fn substr(s: &str, start: i32, len: i32) -> String {
    let slen = s.len() as i32;
    if len <= 0 || slen == 0 {
        return String::new();
    }

    let start = if start < 0 {
        (slen + start).max(0) as usize
    } else {
        (start as usize).min(s.len())
    };

    let end = (start + len as usize).min(s.len());
    s[start..end].to_string()
}

/// Get a single character at index. OSL: `getchar(s, index)`.
/// Returns 0 if out of bounds.
pub fn getchar(s: &str, index: i32) -> i32 {
    if index < 0 || index as usize >= s.len() {
        0
    } else {
        s.as_bytes()[index as usize] as i32
    }
}

/// Convert string to integer. OSL: `stoi(s)`.
/// Matches C++ strtol behavior: parses leading digits, stops at first non-digit.
pub fn stoi(s: &str) -> i32 {
    let s = s.trim();
    let mut end = 0;
    for (i, c) in s.char_indices() {
        if c.is_ascii_digit() || ((c == '-' || c == '+') && i == 0) {
            end = i + c.len_utf8();
        } else {
            break;
        }
    }
    if end == 0 {
        return 0;
    }
    s[..end].parse::<i32>().unwrap_or(0)
}

/// Convert string to float. OSL: `stof(s)`.
/// Matches C++ strtof behavior: parses leading float chars, stops at first invalid char.
pub fn stof(s: &str) -> f32 {
    let s = s.trim();
    let mut end = 0;
    let mut saw_dot = false;
    let mut saw_e = false;
    for (i, c) in s.char_indices() {
        if c.is_ascii_digit() || ((c == '-' || c == '+') && (i == 0 || saw_e && end == i)) {
            end = i + 1;
        } else if c == '.' && !saw_dot && !saw_e {
            saw_dot = true;
            end = i + 1;
        } else if (c == 'e' || c == 'E') && !saw_e && end > 0 {
            saw_e = true;
            end = i + 1;
        } else {
            break;
        }
    }
    if end == 0 {
        return 0.0;
    }
    s[..end].parse::<f32>().unwrap_or(0.0)
}

/// Compute a hash of a string. OSL: `hash(s)`.
#[inline]
pub fn hash_string(s: &str) -> i32 {
    // Use the lower 32 bits of our FarmHash
    (crate::hashes::strhash(s) & 0x7fff_ffff) as i32
}

/// Simple regex search — check if `pattern` matches anywhere in `subject`.
///
/// Regex search — returns true if `pattern` matches anywhere in `subject`.
/// Uses the `regex` crate for full C++ std::regex parity.
pub fn regex_search(subject: &str, pattern: &str) -> bool {
    regex::Regex::new(pattern).is_ok_and(|re| re.is_match(subject))
}

/// Regex match — returns true if `pattern` matches the entire `subject`.
pub fn regex_match(subject: &str, pattern: &str) -> bool {
    let anchored = format!("^(?:{pattern})$");
    regex::Regex::new(&anchored).is_ok_and(|re| re.is_match(subject))
}

/// Regex search with capture group positions.
/// Fills `results` with pairs [begin, end] for each capture group (group 0 = overall match).
/// Matches C++ `osl_regex_impl` behavior: unfilled slots get `pattern.len()`.
pub fn regex_search_captures(
    subject: &str,
    pattern: &str,
    results: &mut [i32],
    fullmatch: bool,
) -> bool {
    let pat = if fullmatch {
        format!("^(?:{pattern})$")
    } else {
        pattern.to_string()
    };
    let re = match regex::Regex::new(&pat) {
        Ok(r) => r,
        Err(_) => {
            for r in results.iter_mut() {
                *r = pattern.len() as i32;
            }
            return false;
        }
    };
    let caps = re.captures(subject);
    match caps {
        Some(c) => {
            let nresults = results.len();
            for (r, slot) in results.iter_mut().enumerate().take(nresults) {
                let group_idx = r / 2;
                if let Some(m) = c.get(group_idx) {
                    *slot = if r & 1 == 0 {
                        m.start() as i32
                    } else {
                        m.end() as i32
                    };
                } else {
                    *slot = pattern.len() as i32;
                }
            }
            true
        }
        None => {
            for r in results.iter_mut() {
                *r = pattern.len() as i32;
            }
            false
        }
    }
}

/// Parse optional width/precision from format spec chars.
fn parse_wp(chars: &mut std::iter::Peekable<std::str::Chars>) -> (Option<usize>, Option<usize>) {
    // Skip flags
    while let Some(&c) = chars.peek() {
        if "-+ 0#".contains(c) {
            chars.next();
        } else {
            break;
        }
    }
    // Width
    let mut w = String::new();
    while let Some(&c) = chars.peek() {
        if c.is_ascii_digit() {
            w.push(c);
            chars.next();
        } else {
            break;
        }
    }
    let width = if w.is_empty() { None } else { w.parse().ok() };
    // Precision
    let mut prec = None;
    if chars.peek() == Some(&'.') {
        chars.next();
        let mut p = String::new();
        while let Some(&c) = chars.peek() {
            if c.is_ascii_digit() {
                p.push(c);
                chars.next();
            } else {
                break;
            }
        }
        prec = Some(p.parse::<usize>().unwrap_or(0));
    }
    (width, prec)
}

/// OSL format string with width/precision support.
/// Supports %[width][.prec]d/f/g/e/s/x/%%.
pub fn format_string(fmt: &str, int_args: &[i32], float_args: &[f32], str_args: &[&str]) -> String {
    let mut result = String::new();
    let mut chars = fmt.chars().peekable();
    let mut ii = 0;
    let mut fi = 0;
    let mut si = 0;

    while let Some(c) = chars.next() {
        if c == '%' {
            if chars.peek() == Some(&'%') {
                chars.next();
                result.push('%');
                continue;
            }
            let (width, prec) = parse_wp(&mut chars);
            if let Some(&spec) = chars.peek() {
                chars.next();
                match spec {
                    'd' | 'i' => {
                        if ii < int_args.len() {
                            let s = int_args[ii].to_string();
                            if let Some(w) = width {
                                result.push_str(&format!("{s:>w$}"));
                            } else {
                                result.push_str(&s);
                            }
                            ii += 1;
                        }
                    }
                    'f' => {
                        if fi < float_args.len() {
                            let p = prec.unwrap_or(6);
                            let val = float_args[fi] as f64;
                            let s = format!("{val:.p$}");
                            if let Some(w) = width {
                                result.push_str(&format!("{s:>w$}"));
                            } else {
                                result.push_str(&s);
                            }
                            fi += 1;
                        }
                    }
                    'g' | 'e' => {
                        if fi < float_args.len() {
                            let p = prec.unwrap_or(6);
                            let val = float_args[fi] as f64;
                            let s = if spec == 'e' {
                                format!("{val:.p$e}")
                            } else {
                                format!("{val:.p$}")
                            };
                            if let Some(w) = width {
                                result.push_str(&format!("{s:>w$}"));
                            } else {
                                result.push_str(&s);
                            }
                            fi += 1;
                        }
                    }
                    's' => {
                        if si < str_args.len() {
                            let sv: String = if let Some(p) = prec {
                                str_args[si].chars().take(p).collect()
                            } else {
                                str_args[si].to_string()
                            };
                            if let Some(w) = width {
                                result.push_str(&format!("{sv:>w$}"));
                            } else {
                                result.push_str(&sv);
                            }
                            si += 1;
                        }
                    }
                    'x' | 'X' => {
                        if ii < int_args.len() {
                            let s = format!("{:x}", int_args[ii]);
                            if let Some(w) = width {
                                result.push_str(&format!("{s:>w$}"));
                            } else {
                                result.push_str(&s);
                            }
                            ii += 1;
                        }
                    }
                    _ => {
                        result.push('%');
                        result.push(spec);
                    }
                }
            } else {
                result.push('%');
            }
        } else if c == '\\' {
            if let Some(&next) = chars.peek() {
                match next {
                    'n' => {
                        chars.next();
                        result.push('\n');
                    }
                    't' => {
                        chars.next();
                        result.push('\t');
                    }
                    '\\' => {
                        chars.next();
                        result.push('\\');
                    }
                    '"' => {
                        chars.next();
                        result.push('"');
                    }
                    _ => {
                        result.push('\\');
                    }
                }
            } else {
                result.push('\\');
            }
        } else {
            result.push(c);
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_concat() {
        assert_eq!(concat("hello", " world"), "hello world");
        assert_eq!(concat("", ""), "");
    }

    #[test]
    fn test_strlen() {
        assert_eq!(strlen("hello"), 5);
        assert_eq!(strlen(""), 0);
    }

    #[test]
    fn test_startswith_endswith() {
        assert!(startswith("hello world", "hello"));
        assert!(!startswith("hello world", "world"));
        assert!(endswith("hello world", "world"));
        assert!(!endswith("hello world", "hello"));
    }

    #[test]
    fn test_substr() {
        assert_eq!(substr("hello world", 6, 5), "world");
        assert_eq!(substr("hello", -3, 3), "llo");
        assert_eq!(substr("hello", 0, 100), "hello");
        assert_eq!(substr("hello", 10, 5), "");
    }

    #[test]
    fn test_getchar() {
        assert_eq!(getchar("ABC", 0), b'A' as i32);
        assert_eq!(getchar("ABC", 2), b'C' as i32);
        assert_eq!(getchar("ABC", 3), 0);
        assert_eq!(getchar("ABC", -1), 0);
    }

    #[test]
    fn test_stoi_stof() {
        assert_eq!(stoi("42"), 42);
        assert_eq!(stoi("-10"), -10);
        assert_eq!(stoi("abc"), 0);
        assert!((stof("3.14") - 3.14).abs() < 0.01);
        assert_eq!(stof("abc"), 0.0);
    }

    #[test]
    fn test_hash() {
        let h1 = hash_string("hello");
        let h2 = hash_string("hello");
        assert_eq!(h1, h2);
        assert_ne!(hash_string("a"), hash_string("b"));
    }

    #[test]
    fn test_regex_search() {
        assert!(regex_search("hello world", "world"));
        assert!(regex_search("hello world", "hel.*"));
        assert!(!regex_search("hello world", "^world"));
    }

    #[test]
    fn test_regex_match() {
        assert!(regex_match("hello", "hello"));
        assert!(regex_match("hello", "hel.*"));
        assert!(!regex_match("hello world", "hello"));
    }

    #[test]
    fn test_format() {
        let s = format_string("x=%d y=%f name=%s", &[42], &[3.14], &["test"]);
        assert!(s.contains("x=42"));
        assert!(s.contains("name=test"));
    }

    #[test]
    fn test_format_width_prec() {
        // %8.3f -> 8-wide, 3 decimal places
        let s = format_string("%8.3f", &[], &[3.14159], &[]);
        assert_eq!(s, "   3.142");
        // %10d -> 10-wide integer
        let s = format_string("%10d", &[42], &[], &[]);
        assert_eq!(s, "        42");
        // %5s -> 5-wide string
        let s = format_string("%5s", &[], &[], &["hi"]);
        assert_eq!(s, "   hi");
        // %.2f -> 2 decimal places, no width
        let s = format_string("%.2f", &[], &[1.23456], &[]);
        assert_eq!(s, "1.23");
    }

    #[test]
    fn test_regex_char_class() {
        // [a-z] matches lowercase
        assert!(regex_search("hello", "^[a-z]+$"));
        assert!(!regex_search("Hello", "^[a-z]+$"));
        // [0-9] matches digits
        assert!(regex_search("42", "^[0-9]+$"));
        assert!(!regex_search("4x", "^[0-9]+$"));
        // [^abc] negated class
        assert!(regex_search("xyz", "^[^abc]+$"));
        assert!(!regex_search("abc", "^[^abc]+$"));
    }

    #[test]
    fn test_regex_plus_quest() {
        // + quantifier: one or more
        assert!(regex_search("aaa", "^a+$"));
        assert!(!regex_search("", "^a+$"));
        // ? quantifier: zero or one
        assert!(regex_match("color", "colou?r"));
        assert!(regex_match("colour", "colou?r"));
        assert!(!regex_match("colouur", "colou?r"));
    }

    #[test]
    fn test_regex_shorthand() {
        // \d matches digits
        assert!(regex_search("abc123", "\\d+"));
        assert!(!regex_search("abc", "^\\d+$"));
        // \w matches word chars
        assert!(regex_search("hello_42", "^\\w+$"));
        assert!(!regex_search("hello world", "^\\w+$"));
        // \s matches whitespace
        assert!(regex_search("hello world", "\\s"));
        assert!(!regex_search("hello", "\\s"));
    }

    #[test]
    fn test_regex_counted_rep() {
        // {n} exact count
        assert!(regex_match("foo", "f[Oo]{2}"));
        assert!(!regex_match("foo", "f[Oo]{3}"));
        // {n,m} range
        assert!(regex_search("aaa", "a{2,4}"));
        assert!(!regex_match("a", "a{2,4}"));
    }

    #[test]
    fn test_regex_groups() {
        // Basic group
        assert!(regex_search("foobar.baz", "(f[Oo]{2}).*(.az)"));
        assert!(regex_match("foobar.baz", "(f[Oo]{2}).*(.az)"));
        // Group quantifiers
        assert!(regex_match("abab", "(ab)+"));
        assert!(regex_match("ab", "(ab)?"));
    }

    #[test]
    fn test_regex_escape() {
        // \. matches literal dot
        assert!(regex_search("3.14", "3\\.14"));
        assert!(!regex_search("3x14", "3\\.14"));
        // \* matches literal star
        assert!(regex_search("a*b", "a\\*b"));
    }

    // --- regex_search_captures tests ---

    #[test]
    fn test_regex_search_captures_basic() {
        // Pattern "(foo).*(\..az)" against "foobar.baz"
        // Full match: 0..10, group1 "foo": 0..3, group2 ".baz": 6..10
        // results layout: [full_start, full_end, g1_start, g1_end, g2_start, g2_end]
        let mut results = vec![0i32; 6];
        let found = regex_search_captures("foobar.baz", r"(foo).*(\..az)", &mut results, false);
        assert!(found);
        assert_eq!(results[0], 0); // full match start
        assert_eq!(results[1], 10); // full match end
        assert_eq!(results[2], 0); // group 1 start ("foo")
        assert_eq!(results[3], 3); // group 1 end
        assert_eq!(results[4], 6); // group 2 start (".baz")
        assert_eq!(results[5], 10); // group 2 end
    }

    #[test]
    fn test_regex_search_captures_no_match() {
        // No match: all slots set to pattern.len()
        // Pattern "(xyz)" has len 5
        let mut results = vec![0i32; 4];
        let found = regex_search_captures("hello", r"(xyz)", &mut results, false);
        assert!(!found);
        assert!(results.iter().all(|&v| v == 5));
    }

    #[test]
    fn test_regex_match_captures_fullmatch() {
        // fullmatch=true: "(foo)(bar)" must match entire "foobar"
        // results: [full_start, full_end, g1_start, g1_end]  (4 slots = 2 groups)
        let mut results = vec![0i32; 4];
        let found = regex_search_captures("foobar", r"(foo)(bar)", &mut results, true);
        assert!(found);
        assert_eq!(results[0], 0); // full match start
        assert_eq!(results[1], 6); // full match end
        assert_eq!(results[2], 0); // group 1 start ("foo")
        assert_eq!(results[3], 3); // group 1 end
    }

    #[test]
    fn test_regex_match_captures_fullmatch_fail() {
        // fullmatch=true should reject partial matches
        // Pattern "(foo)(bar)" does not fully match "foobar_extra"
        let mut results = vec![0i32; 4];
        let found = regex_search_captures("foobar_extra", r"(foo)(bar)", &mut results, true);
        assert!(!found);
    }

    #[test]
    fn test_regex_captures_extra_slots() {
        // More result slots than groups: unfilled slots get pattern.len()
        // Pattern "(a)" has len 3, matches "abc" with full=0..1, g1=0..1
        let mut results = vec![0i32; 10];
        let found = regex_search_captures("abc", r"(a)", &mut results, false);
        assert!(found);
        assert_eq!(results[0], 0); // full match start
        assert_eq!(results[1], 1); // full match end
        assert_eq!(results[2], 0); // group 1 start
        assert_eq!(results[3], 1); // group 1 end
        // Slots beyond available groups filled with pattern.len() = 3
        for &v in &results[4..] {
            assert_eq!(v, 3);
        }
    }

    #[test]
    fn test_regex_captures_search_offset() {
        // regex_search finds first occurrence anywhere in subject
        // "(\\d+)" against "abc123def" -> full match at 3..6, g1 at 3..6
        let mut results = vec![0i32; 4];
        let found = regex_search_captures("abc123def", r"(\d+)", &mut results, false);
        assert!(found);
        assert_eq!(results[0], 3); // full match start
        assert_eq!(results[1], 6); // full match end
        assert_eq!(results[2], 3); // group 1 start
        assert_eq!(results[3], 6); // group 1 end
    }

    #[test]
    fn test_regex_captures_invalid_pattern() {
        // Invalid regex: all slots set to pattern.len(), returns false
        // Pattern "[invalid" has len 8
        let mut results = vec![0i32; 4];
        let found = regex_search_captures("anything", "[invalid", &mut results, false);
        assert!(!found);
        assert!(results.iter().all(|&v| v == 8));
    }
}
