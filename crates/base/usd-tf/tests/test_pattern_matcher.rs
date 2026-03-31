// Port of testenv/patternMatcher.cpp

use usd_tf::pattern_matcher::PatternMatcher;

/// Verify that `matcher.matches(query)` returns `expected`.
/// Mirrors C++ `status &= pm.Match(...)` / `status &= !pm.Match(...)`.
fn check(matcher: &PatternMatcher, query: &str, expected: bool) {
    let (result, _err) = matcher.matches_with_error(query);
    assert_eq!(
        result,
        expected,
        "pattern={:?} glob={} case_sensitive={} query={:?}: expected {}",
        matcher.pattern(),
        matcher.is_glob_pattern(),
        matcher.is_case_sensitive(),
        query,
        expected
    );
}

// ---------------------------------------------------------------------------
// Glob tests — mirrors the first block in Test_TfPatternMatcher
// ---------------------------------------------------------------------------

#[test]
fn glob_case_sensitive_matches_lowercase() {
    // SetPattern("oast"), SetIsGlobPattern(true), SetIsCaseSensitive(true)
    // Match("i like toast") → true  (unanchored: "oast" is substring)
    let mut pm = PatternMatcher::empty();
    pm.set_pattern("oast");
    pm.set_is_glob_pattern(true);
    pm.set_case_sensitive(true);
    check(&pm, "i like toast", true);
}

#[test]
fn glob_case_sensitive_does_not_match_mixed_case() {
    // Same settings, Match("i like ToaST") → false (case-sensitive)
    let mut pm = PatternMatcher::empty();
    pm.set_pattern("oast");
    pm.set_is_glob_pattern(true);
    pm.set_case_sensitive(true);
    check(&pm, "i like ToaST", false);
}

#[test]
fn glob_trailing_backslash_invalid() {
    // SetPattern("oast\\") — trailing backslash makes the glob invalid,
    // so Match should return false regardless of the input.
    let mut pm = PatternMatcher::empty();
    pm.set_pattern("oast\\");
    pm.set_is_glob_pattern(true);
    pm.set_case_sensitive(true);
    // In C++ an invalid regex causes Match to return false.
    let (result, _) = pm.matches_with_error("i like toast");
    assert!(
        !result,
        "invalid glob with trailing backslash must not match"
    );
}

// ---------------------------------------------------------------------------
// Regex tests — mirrors the TfPatternMatcher dt block
// ---------------------------------------------------------------------------

/// Loose date/time pattern — anchored at the start, optional time suffix.
/// `^[0-9]{4}/[0-9]{2}/[0-9]{2}(:[0-9]{2}:[0-9]{2}:[0-9]{2})?`
fn date_time_matcher() -> PatternMatcher {
    // is_glob=false → raw regex; case_sensitive does not matter for digits
    PatternMatcher::new(
        r"^[0-9]{4}/[0-9]{2}/[0-9]{2}(:[0-9]{2}:[0-9]{2}:[0-9]{2})?",
        false,
        false,
    )
}

#[test]
fn regex_date_matches_date_only() {
    check(&date_time_matcher(), "2009/01/01", true);
}

#[test]
fn regex_date_matches_date_with_time() {
    check(&date_time_matcher(), "2009/01/01:12:34:56", true);
}

#[test]
fn regex_date_rejects_wrong_order() {
    // "01/01/2009" — month/day/year order, no match
    check(&date_time_matcher(), "01/01/2009", false);
}

// ---------------------------------------------------------------------------
// Additional coverage
// ---------------------------------------------------------------------------

#[test]
fn glob_star_wildcard() {
    let pm = PatternMatcher::new("*.txt", false, true);
    assert!(pm.matches("file.txt"));
    assert!(!pm.matches("file.rs"));
}

#[test]
fn glob_question_wildcard() {
    let pm = PatternMatcher::new("file?.txt", false, true);
    assert!(pm.matches("file1.txt"));
    assert!(!pm.matches("file12.txt"));
    assert!(!pm.matches("file.txt"));
}

#[test]
fn glob_case_insensitive() {
    let pm = PatternMatcher::new("hello", false, true);
    assert!(pm.matches("HELLO"));
    assert!(pm.matches("hello"));
}

#[test]
fn empty_pattern_is_invalid() {
    let pm = PatternMatcher::empty();
    assert!(!pm.is_valid());
    let (result, err) = pm.matches_with_error("anything");
    assert!(!result);
    assert!(err.is_some());
}
