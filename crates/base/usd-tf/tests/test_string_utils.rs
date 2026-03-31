// Port of pxr/base/tf/testenv/stringUtils.cpp
// Tests for TfStringUtils — numbers, predicates, strings, tokens, XML escape.

use usd_tf::string_utils::*;

// ---------------------------------------------------------------------------
// TestNumbers
// ---------------------------------------------------------------------------

#[test]
fn test_string_to_double_basics() {
    assert_eq!(string_to_double("") as f32, 0.0f32);
    assert_eq!(string_to_double("blah") as f32, 0.0f32);
    let v = string_to_double("-") as f32;
    assert!(v == 0.0f32 || v == -0.0f32);
    assert_eq!(string_to_double("1.2") as f32, 1.2f32);
    assert_eq!(string_to_double("1") as f32, 1.0f32);
    assert_eq!(string_to_double("-5000001") as f32, -5000001.0f32);
    assert_eq!(string_to_double("0.123") as f32, 0.123f32);
    assert_eq!(string_to_double("-.123") as f32, -0.123f32);
    assert_eq!(string_to_double("-1e3") as f32, -1e3f32);
    assert_eq!(string_to_double("1e6") as f32, 1e6f32);
    assert_eq!(string_to_double("-1E-1") as f32, -1E-1f32);
}

#[test]
fn test_int_to_string() {
    assert_eq!(int_to_string(1), "1");
    assert_eq!(int_to_string(1024), "1024");
    assert_eq!(int_to_string(0), "0");
    assert_eq!(int_to_string(-22), "-22");
}

#[test]
fn test_double_roundtrip() {
    // Representative values that broke under prior implementations.
    assert_eq!(string_to_double(&stringify_double(0.1)), 0.1);
    assert_eq!(
        string_to_double(&stringify_double(0.336316384899143)),
        0.336316384899143
    );
    assert_eq!(string_to_double(&stringify_float(0.1f32)) as f32, 0.1f32);
    assert_eq!(
        string_to_double(&stringify_float(0.84066f32)) as f32,
        0.84066f32
    );
}

#[test]
fn test_double_to_string_buffer() {
    // double_to_string with emit_trailing_zero=true
    let s = double_to_string(-1.1111111111111113e-308, true);
    assert_eq!(s, "-1.1111111111111113e-308");
}

// ---------------------------------------------------------------------------
// TestPreds — starts_with / ends_with
// ---------------------------------------------------------------------------

#[test]
fn test_starts_with() {
    assert!(starts_with("  ", "  "));
    assert!(starts_with("abc", "ab"));
    assert!(starts_with("xyz", "xyz"));
    assert!(starts_with("a little bit longer string", "a little"));
    assert!(starts_with("anything", ""));
    assert!(!starts_with("", " "));
    assert!(!starts_with("abc", "bc"));
}

#[test]
fn test_ends_with() {
    assert!(ends_with("  ", "  "));
    assert!(ends_with("abc", "bc"));
    assert!(ends_with("xyz", "xyz"));
    assert!(ends_with("a little bit longer string", " string"));
    assert!(ends_with("anything", ""));
    assert!(!ends_with("", " "));
    assert!(!ends_with("abc", "ab"));
}

// ---------------------------------------------------------------------------
// TestPreds — DictionaryLessThan
// ---------------------------------------------------------------------------

fn dict_lt(a: &str, b: &str) -> bool {
    dictionary_less_than(a, b)
}

#[test]
fn test_dictionary_less_than_basic() {
    assert!(dict_lt("ring", "robot"));
    assert!(!dict_lt("robot", "ring"));
    assert!(!dict_lt("Alex", "aardvark"));
    assert!(dict_lt("aardvark", "Alex"));
    assert!(dict_lt("Alex", "AMD"));
    assert!(!dict_lt("AMD", "Alex"));
}

#[test]
fn test_dictionary_less_than_numeric() {
    assert!(dict_lt("1", "15"));
    assert!(!dict_lt("15", "1"));
    assert!(dict_lt("1998", "1999"));
    assert!(!dict_lt("1999", "1998"));
    assert!(dict_lt("Worker8", "Worker11"));
    assert!(!dict_lt("Worker11", "Worker8"));
    assert!(dict_lt("agent007", "agent222"));
    assert!(!dict_lt("agent222", "agent007"));
    assert!(dict_lt("agent007", "agent0007"));
    assert!(dict_lt("agent7", "agent07"));
    assert!(!dict_lt("agent07", "agent07"));
    assert!(dict_lt("0", "00"));
    assert!(dict_lt("1", "01"));
    assert!(!dict_lt("2", "01"));
}

#[test]
fn test_dictionary_less_than_mixed_segments() {
    assert!(dict_lt("foo001bar001abc", "foo001bar002abc"));
    assert!(dict_lt("foo001bar01abc", "foo001bar001abc"));
    assert!(!dict_lt("foo001bar002abc", "foo001bar001abc"));
    assert!(dict_lt("foo00001bar0002abc", "foo001bar002xyz"));
    assert!(!dict_lt("foo00001bar0002xyz", "foo001bar002abc"));
    assert!(dict_lt("foo1bar02", "foo01bar2"));
    assert!(dict_lt("agent007", "agent8"));
    assert!(dict_lt("agent007", "agent222"));
}

#[test]
fn test_dictionary_less_than_case_and_special() {
    assert!(!dict_lt("GOTO8", "goto7"));
    assert!(dict_lt("goto7", "GOTO8"));
    assert!(dict_lt("!", "$"));
    assert!(!dict_lt("$", "!"));
    assert!(!dict_lt("foo", "foo")); // equal → false
    assert!(dict_lt("aa", "aaa"));
    assert!(!dict_lt("aaa", "aa"));
}

#[test]
fn test_dictionary_less_than_leading_zeros() {
    assert!(dict_lt("0a", "00A"));
    assert!(!dict_lt("00A", "0a"));
    assert!(dict_lt("000a", "0000a"));
    assert!(!dict_lt("0000a", "000a"));
}

#[test]
fn test_dictionary_less_than_underscores() {
    assert!(dict_lt("foo_bar", "foobar"));
    assert!(!dict_lt("foobar", "foo_bar"));
    assert!(dict_lt("_foobar", "foobar"));
    assert!(!dict_lt("foobar", "_foobar"));
    assert!(dict_lt("__foobar", "_foobar"));
    assert!(!dict_lt("_foobar", "__foobar"));
    assert!(dict_lt("Foo_Bar", "FooBar"));
    assert!(!dict_lt("FooBar", "Foo_Bar"));
    assert!(dict_lt("_FooBar", "FooBar"));
    assert!(!dict_lt("FooBar", "_FooBar"));
    assert!(dict_lt("__FooBar", "_FooBar"));
    assert!(!dict_lt("_FooBar", "__FooBar"));
}

#[test]
fn test_dictionary_less_than_large_numbers() {
    assert!(dict_lt("abc012300", "abc000012300"));
    assert!(!dict_lt("abc0000123000", "abc0123000"));
    assert!(dict_lt("0345678987654321234567", "03456789876543212345670"));
    assert!(!dict_lt(
        "03456789876543212345670",
        "0345678987654321234567"
    ));
    assert!(dict_lt("0345678987654321234567", "0345678987654322234567"));
    assert!(!dict_lt("0345678987654322234567", "0345678987654321234567"));
    assert!(dict_lt(
        "XXX_0345678987654321234567",
        "XXX_03456789876543212345670"
    ));
    assert!(!dict_lt(
        "XXX_03456789876543212345670",
        "XXX_0345678987654321234567"
    ));
    assert!(dict_lt(
        "XXX_0345678987654321234567",
        "XXX_0345678987654322234567"
    ));
    assert!(!dict_lt(
        "XXX_0345678987654322234567",
        "XXX_0345678987654321234567"
    ));
}

#[test]
fn test_dictionary_less_than_colon_underscore() {
    assert!(!dict_lt(
        "primvars:curveHierarchy__id",
        "primvars:curveHierarchy:id"
    ));
    assert!(dict_lt(
        "primvars:curveHierarchy:id",
        "primvars:curveHierarchy__id"
    ));
}

#[test]
fn test_dictionary_less_than_utf8() {
    // U+00FC (ü) vs U+0061 (a) — multi-byte > single ASCII byte
    assert!(!dict_lt("ü", "a"));
    // U+1300A (𓀊) vs U+0041 (A)
    assert!(!dict_lt("𓀊", "A"));
    // U+222B (∫) vs U+003D (=)
    assert!(!dict_lt("∫", "="));
    // U+0F22 (༢) vs U+0036 (6)
    assert!(!dict_lt("༢", "6"));
    // U+0F22 (༢) vs U+0F28 (༨) — both multi-byte, lower code point first
    assert!(dict_lt("༢", "༨"));
    assert!(dict_lt("_", "㤼"));
    assert!(dict_lt("_a", "_a㤼"));
    assert!(dict_lt("6", "_a"));
    assert!(!dict_lt("2_༢1", "2_༢"));
    assert!(!dict_lt("∫∫", "∫="));
    // U+03C7 (χ) U+03C0 (π) in numeric context
    assert!(!dict_lt("a00χ", "a0π"));
    assert!(!dict_lt("00χ", "0π"));
    // Loop tests with U+393B/C/A
    assert!(dict_lt("foo001bar001abc㤻", "foo001bar001abc㤼"));
    assert!(!dict_lt("foo001㤻bar01abc", "foo001㤺bar001abc"));
    assert!(!dict_lt("foo001㤻bar001abc", "foo001㤻bar001abc"));
    assert!(!dict_lt("foo00001bar0002ü", "foo001bar002abc"));
    assert!(dict_lt("üfoo", "㤻foo"));
}

// ---------------------------------------------------------------------------
// TestPreds — is_valid_identifier
// ---------------------------------------------------------------------------

#[test]
fn test_is_valid_identifier_valid() {
    assert!(is_valid_identifier("f"));
    assert!(is_valid_identifier("foo"));
    assert!(is_valid_identifier("foo1"));
    assert!(is_valid_identifier("_foo"));
    assert!(is_valid_identifier("_foo1"));
    assert!(is_valid_identifier("__foo__"));
    assert!(is_valid_identifier("__foo1__"));
    assert!(is_valid_identifier("__foo1__2"));
    assert!(is_valid_identifier("_"));
    assert!(is_valid_identifier("_2"));
}

#[test]
fn test_is_valid_identifier_invalid() {
    assert!(!is_valid_identifier(""));
    assert!(!is_valid_identifier("1"));
    assert!(!is_valid_identifier("2foo"));
    assert!(!is_valid_identifier("1_foo"));
    assert!(!is_valid_identifier("13_foo2"));
    assert!(!is_valid_identifier(" "));
    assert!(!is_valid_identifier(" foo"));
    assert!(!is_valid_identifier(" _foo\n "));
    assert!(!is_valid_identifier(" _foo32 \t   "));
    assert!(!is_valid_identifier("$"));
    assert!(!is_valid_identifier("\x07")); // \a bell
    assert!(!is_valid_identifier("foo$"));
    assert!(!is_valid_identifier("_foo$"));
    assert!(!is_valid_identifier(" _foo$"));
    assert!(!is_valid_identifier("foo bar"));
    assert!(!is_valid_identifier("\"foo\""));
}

// ---------------------------------------------------------------------------
// TestStrings — case conversion
// ---------------------------------------------------------------------------

#[test]
fn test_to_lower() {
    assert_eq!(to_lower("  "), "  ");
    assert_eq!(to_lower("lower"), "lower");
    assert_eq!(to_lower("LOWER"), "lower");
    assert_eq!(to_lower("LOWer"), "lower");
    assert_eq!(to_lower("LOWer@123"), "lower@123");
}

#[test]
fn test_to_upper() {
    assert_eq!(to_upper("upper"), "UPPER");
    assert_eq!(to_upper("UPPER"), "UPPER");
    assert_eq!(to_upper("UPPer"), "UPPER");
    assert_eq!(to_upper("UPPer@123"), "UPPER@123");
}

#[test]
fn test_capitalize() {
    assert_eq!(capitalize("Already"), "Already");
    assert_eq!(capitalize("notyet"), "Notyet");
    assert_eq!(capitalize("@@@@"), "@@@@");
    assert_eq!(capitalize(""), "");
}

#[test]
fn test_to_lower_ascii() {
    assert_eq!(to_lower_ascii("PIXAR"), to_lower_ascii("pixar"));
    assert_eq!(to_lower_ascii("PiXaR"), to_lower_ascii("pixar"));
    // Non-ASCII Greek letters are NOT case-folded by the ASCII-only function.
    assert_eq!(to_lower_ascii("ΠΙΞΑΡ"), "ΠΙΞΑΡ");
    // Mixture: only ASCII letters are lowered.
    assert_eq!(to_lower_ascii("ΠΙΞΑΡ ≈ PIXAR"), "ΠΙΞΑΡ ≈ pixar");
}

// ---------------------------------------------------------------------------
// TestStrings — suffix / prefix helpers
// ---------------------------------------------------------------------------

#[test]
fn test_get_suffix() {
    assert_eq!(get_suffix("file.ext", '.'), "ext");
    assert_eq!(get_suffix("here are some words", ' '), "words");
    assert_eq!(get_suffix("0words", '0'), "words");
    assert_eq!(get_suffix("A@B@C", '@'), "C");
    assert_eq!(get_suffix("nothing", ' '), "");
    assert_eq!(get_suffix("nothing", '\0'), "");
}

#[test]
fn test_get_before_suffix() {
    assert_eq!(get_before_suffix("file.ext", '.'), "file");
    assert_eq!(
        get_before_suffix("here are some words", ' '),
        "here are some"
    );
    assert_eq!(get_before_suffix("0words", '0'), "");
    assert_eq!(get_before_suffix("A@B@C", '@'), "A@B");
    assert_eq!(get_before_suffix("nothing", ' '), "nothing");
    assert_eq!(get_before_suffix("nothing", '\0'), "nothing");
}

#[test]
fn test_get_base_name() {
    assert_eq!(get_base_name(""), "");
    assert_eq!(get_base_name("/foo/bar"), "bar");
    assert_eq!(get_base_name("/foo/bar/"), "bar");
    assert_eq!(get_base_name("../some-dir/bar"), "bar");
    assert_eq!(get_base_name("bar"), "bar");
}

#[test]
fn test_get_path_name() {
    assert_eq!(get_path_name(""), "");
    assert_eq!(get_path_name("/"), "/");
    assert_eq!(get_path_name("/foo/bar"), "/foo/");
    assert_eq!(get_path_name("../some-dir/bar"), "../some-dir/");
    assert_eq!(get_path_name("bar"), "");
}

// ---------------------------------------------------------------------------
// TestStrings — trim (C++ TfStringTrimRight/Left/Trim with char set param)
// ---------------------------------------------------------------------------

#[test]
fn test_trim_right() {
    // trim_right strips trailing whitespace; trim_chars strips a given set from both ends.
    // The C++ TfStringTrimRight("", " ") and TfStringTrimRight("x", "") map to trim_chars.
    assert_eq!(trim_chars("", " "), "");
    assert_eq!(trim_right("to be trimmed"), "to be trimmed");
    assert_eq!(trim_chars("to be trimmed", "x"), "to be trimmed");
    // trailing space stripped
    assert_eq!(trim_right(" to be trimmed "), " to be trimmed");
    assert_eq!(trim_chars("  to be trimmed  ", " "), "to be trimmed");
}

#[test]
fn test_trim_left() {
    assert_eq!(trim_chars("", " "), "");
    assert_eq!(trim_left("to be trimmed"), "to be trimmed");
    assert_eq!(trim_chars("to be trimmed", "x"), "to be trimmed");
    assert_eq!(trim_left(" to be trimmed "), "to be trimmed ");
    assert_eq!(trim_chars("  to be trimmed  ", " "), "to be trimmed");
}

#[test]
fn test_trim() {
    assert_eq!(trim_chars("", " "), "");
    assert_eq!(trim("to be trimmed"), "to be trimmed");
    assert_eq!(trim_chars("to be trimmed", "x"), "to be trimmed");
    assert_eq!(trim(" to be trimmed "), "to be trimmed");
    assert_eq!(trim_chars("  to be trimmed  ", " "), "to be trimmed");
    assert_eq!(trim_chars(" to be trimmed ", "x "), "to be trimmed");
    assert_eq!(trim_chars("_to be trimmed ", "_ "), "to be trimmed");
}

// ---------------------------------------------------------------------------
// TestStrings — replace / common_prefix
// ---------------------------------------------------------------------------

#[test]
fn test_replace() {
    assert_eq!(replace("an old string", "n old", " new"), "a new string");
    assert_eq!(replace("remove", "remove", ""), "");
    assert_eq!(replace("12121", "21", "31"), "13131");
    assert_eq!(replace("aaaa", "aa", "b"), "bb");
    assert_eq!(replace("no more spaces", " ", "_"), "no_more_spaces");
    // Case-sensitive: "cap" not found in "Capital"
    assert_eq!(replace("Capital", "cap", "zap"), "Capital");
    // Empty from-string → no change
    assert_eq!(replace("string", "", "number"), "string");
    assert_eq!(replace("string", "str", "str"), "string");
}

#[test]
fn test_common_prefix() {
    assert_eq!(common_prefix("", ""), "");
    assert_eq!(common_prefix("a", ""), "");
    assert_eq!(common_prefix("", "b"), "");
    assert_eq!(common_prefix("a", "b"), "");
    assert_eq!(common_prefix("a", "a"), "a");
    assert_eq!(common_prefix("abracadabra", "abracababra"), "abraca");
    assert_eq!(common_prefix("aabcd", "aaabcd"), "aa");
    assert_eq!(common_prefix("aabcdefg", "aabcd"), "aabcd");
}

// ---------------------------------------------------------------------------
// TestStrings — stringify / unstringify
// ---------------------------------------------------------------------------

#[test]
fn test_stringify_bool() {
    assert_eq!(stringify_bool(true), "true");
    assert_eq!(stringify_bool(false), "false");
    assert!(unstringify_bool("true"));
    assert!(!unstringify_bool("false"));
}

#[test]
fn test_stringify_locale_agnostic() {
    // Must use '.' as decimal separator regardless of locale.
    assert_eq!(stringify_double(1000.56), "1000.56");
    assert_eq!(stringify_double(1.1), "1.1");
}

// ---------------------------------------------------------------------------
// TestStrings — escape_string
// ---------------------------------------------------------------------------

#[test]
fn test_escape_string() {
    // C++ TfEscapeString processes C-style backslash sequences in the input.
    assert_eq!(escape_string("\\\\"), "\\");
    assert_eq!(escape_string("new\\nline"), "new\nline");
    assert_eq!(escape_string("two\\nnew\\nlines"), "two\nnew\nlines");
    assert_eq!(escape_string("a\\ttab"), "a\ttab");
    assert_eq!(escape_string("\\a\\b"), "\x07\x08");
    assert_eq!(escape_string("\\f\\n"), "\x0C\n");
    assert_eq!(escape_string("\\r\\v"), "\r\x0B");
    // Unknown escapes strip the backslash: \c → 'c', \d → 'd'
    assert_eq!(escape_string("\\c \\d"), "c d");
    // Hex escapes (stop after exactly two hex digits past the \x)
    assert_eq!(escape_string("\\xB"), "\x0B");
    assert_eq!(escape_string("\\xab"), "\u{ab}");
    // \x01f → byte 0x01 then literal 'f'
    assert_eq!(escape_string("\\x01f"), "\x01f");
    // \x008d → byte 0x00 then literal "8d"
    assert_eq!(escape_string("\\x008d"), "\x008d");
    assert_eq!(escape_string("x\\x0x"), "x\x00x");
    // Octal escapes (up to 3 octal digits, value must fit in one byte)
    assert_eq!(escape_string("\\5"), "\x05");
    assert_eq!(escape_string("\\70"), "\x38"); // octal 70 = decimal 56
    assert_eq!(escape_string("\\11z"), "\x09z");
    assert_eq!(escape_string("\\007"), "\u{7}"); // octal 007 = 0x07 (BEL)
    assert_eq!(escape_string("\\008"), "\u{0}8"); // \0 = null, then '8'
    assert_eq!(escape_string("\\010"), "\u{8}"); // octal 010 = 8 (BS)
    assert_eq!(escape_string("\\0077"), "\u{7}7"); // octal 007 then '7'
    assert_eq!(escape_string("\\00107"), "\u{1}07"); // octal 001 then "07"
    assert_eq!(escape_string("\\005107"), "\u{5}107"); // octal 005 then "107"
}

// ---------------------------------------------------------------------------
// TestStrings — cat_paths
// ---------------------------------------------------------------------------

#[test]
fn test_cat_paths() {
    assert_eq!(cat_paths("foo", "bar"), "foo/bar");
    assert_eq!(cat_paths("foo/crud", "../bar"), "foo/bar");
    assert_eq!(cat_paths("foo", "../bar"), "bar");
    assert_eq!(cat_paths("/foo", "../bar"), "/bar");
    assert_eq!(cat_paths("foo/crud/crap", "../bar"), "foo/crud/bar");
}

// ---------------------------------------------------------------------------
// TestTokens — tokenize / join
// ---------------------------------------------------------------------------

#[test]
fn test_string_join_empty() {
    let empty: Vec<&str> = vec![];
    assert_eq!(join(&empty, " "), "");
}

#[test]
fn test_tokenize_to_set() {
    let set = tokenize_to_set(" to   be   tokens ", " ");
    assert_eq!(set.len(), 3);

    let set2 = tokenize_to_set(" to   be   tokens", " ");
    assert_eq!(set2.len(), 3);
}

#[test]
fn test_tokenize_and_join() {
    let tokens = tokenize(" to   be   tokens ", " ");
    assert_eq!(tokens.len(), 3);
    assert_eq!(join(&tokens, " "), "to be tokens");

    let tokens2 = tokenize("A1B2C3", "123");
    assert_eq!(tokens2.len(), 3);
    assert_eq!(join(&tokens2, ""), "ABC");

    // Empty delimiter → treat entire string as one token
    let tokens3 = tokenize("no tokens", "");
    assert_eq!(tokens3.len(), 1);
    assert_eq!(join(&tokens3, ""), "no tokens");

    let tokens4 = tokenize("no tokens", "xyz");
    assert_eq!(tokens4.len(), 1);
    assert_eq!(join(&tokens4, " "), "no tokens");
}

// ---------------------------------------------------------------------------
// TestTokens — quoted_string_tokenize (returns Result)
// ---------------------------------------------------------------------------

#[test]
fn test_quoted_string_tokenize_basic() {
    let tokens = quoted_string_tokenize("\"no tokens\"", " ").unwrap();
    assert_eq!(tokens.len(), 1);
    assert_eq!(join(&tokens, " "), "no tokens");
}

#[test]
fn test_quoted_string_tokenize_adjacent() {
    let tokens = quoted_string_tokenize("  foo\"no tokens\"", " ").unwrap();
    assert_eq!(tokens.len(), 1);
    assert_eq!(join(&tokens, " "), "foono tokens");
}

#[test]
fn test_quoted_string_tokenize_error_quote_delimiter() {
    // Delimiter and quote char are the same → error
    assert!(quoted_string_tokenize("\"no tokens\"", "\"").is_err());
}

#[test]
fn test_quoted_string_tokenize_unterminated_quote() {
    let result = quoted_string_tokenize("\"no tokens", " ");
    assert!(result.is_err());
}

#[test]
fn test_quoted_string_tokenize_numeric() {
    let tokens = quoted_string_tokenize("A1B2C3", "123").unwrap();
    assert_eq!(tokens.len(), 3);
    assert_eq!(join(&tokens, ""), "ABC");
}

#[test]
fn test_quoted_string_tokenize_escaped_quotes() {
    let tokens = quoted_string_tokenize("\"a \\\"b\\\" c\" d", " ").unwrap();
    assert_eq!(tokens.len(), 2);
    assert_eq!(join(&tokens, " "), "a \"b\" c d");
}

#[test]
fn test_quoted_string_tokenize_two_quoted_tokens() {
    let tokens = quoted_string_tokenize(" \"there are\" \"two tokens\" ", " ").unwrap();
    assert_eq!(tokens.len(), 2);
    assert_eq!(join(&tokens, " "), "there are two tokens");
}

#[test]
fn test_quoted_string_tokenize_adjacent_quotes_one_token() {
    let tokens = quoted_string_tokenize("\"there is\"\" one token\"", " ").unwrap();
    assert_eq!(tokens.len(), 1);
    assert_eq!(join(&tokens, " "), "there is one token");
}

#[test]
fn test_quoted_string_tokenize_escaped_quotes_split() {
    // Backslash-escaped quotes outside a quoted run are literal quote chars.
    let tokens = quoted_string_tokenize("\\\"this_gets_split\\\"", "_").unwrap();
    assert_eq!(tokens.len(), 3);
    assert_eq!(join(&tokens, " "), "\"this gets split\"");
}

#[test]
fn test_quoted_string_tokenize_quoted_no_split() {
    // Inside quotes, underscore is not a delimiter.
    let tokens = quoted_string_tokenize("\"\\\"this_doesn't\\\"\"", "_").unwrap();
    assert_eq!(tokens.len(), 1);
    assert_eq!(join(&tokens, " "), "\"this_doesn't\"");
}

#[test]
fn test_quoted_string_tokenize_mixed_quotes() {
    let tokens = quoted_string_tokenize("\"'nothing' `to` \\\"split\\\"\"", " ").unwrap();
    assert_eq!(tokens.len(), 1);
    assert_eq!(join(&tokens, " "), "'nothing' `to` \"split\"");
}

#[test]
fn test_quoted_string_tokenize_single_quote_escape() {
    let tokens = quoted_string_tokenize("'esc\\\"' \\\"aped", " ").unwrap();
    assert_eq!(tokens.len(), 2);
    assert_eq!(join(&tokens, " "), "esc\" \"aped");
}

// ---------------------------------------------------------------------------
// TestTokens — matched_string_tokenize (returns Result)
// ---------------------------------------------------------------------------

#[test]
fn test_matched_string_tokenize_same_delimiter_error() {
    // open == close → error
    assert!(matched_string_tokenize("{", '{', '{', '\0').is_err());
}

#[test]
fn test_matched_string_tokenize_close_before_open_error() {
    assert!(matched_string_tokenize("}garble{", '{', '}', '\0').is_err());
}

#[test]
fn test_matched_string_tokenize_unmatched_open_error() {
    assert!(matched_string_tokenize("{garble} {", '{', '}', '\0').is_err());
}

#[test]
fn test_matched_string_tokenize_unmatched_close_error() {
    assert!(matched_string_tokenize("{garble} }", '{', '}', '\0').is_err());
}

#[test]
fn test_matched_string_tokenize_degenerate() {
    // Single unmatched delimiters produce empty or error results.
    assert!(
        matched_string_tokenize("{", '{', '}', '\0')
            .map(|v| v.is_empty())
            .unwrap_or(true)
    );
    assert!(
        matched_string_tokenize("}", '{', '}', '\0')
            .map(|v| v.is_empty())
            .unwrap_or(true)
    );
    assert!(
        matched_string_tokenize("}{}", '{', '}', '\0')
            .map(|v| v.is_empty())
            .unwrap_or(true)
    );
    assert!(
        matched_string_tokenize("{}{", '{', '}', '\0')
            .map(|v| v.is_empty())
            .unwrap_or(true)
    );
    assert!(
        matched_string_tokenize("{}}", '{', '}', '\0')
            .map(|v| v.is_empty())
            .unwrap_or(true)
    );
    assert!(
        matched_string_tokenize("{{}", '{', '}', '\0')
            .map(|v| v.is_empty())
            .unwrap_or(true)
    );
    assert!(
        matched_string_tokenize("{whoops", '{', '}', '\0')
            .map(|v| v.is_empty())
            .unwrap_or(true)
    );
    assert!(
        matched_string_tokenize("none!", '{', '}', '\0')
            .map(|v| v.is_empty())
            .unwrap_or(true)
    );
}

#[test]
fn test_matched_string_tokenize_nested() {
    let tokens = matched_string_tokenize("{test {test} test}", '{', '}', '\0').unwrap();
    assert_eq!(tokens.len(), 1);
    assert_eq!(join(&tokens, " "), "test {test} test");
}

#[test]
fn test_matched_string_tokenize_two_tokens() {
    let tokens = matched_string_tokenize("{foo} {bar}", '{', '}', '\0').unwrap();
    assert_eq!(tokens.len(), 2);
    assert_eq!(join(&tokens, " "), "foo bar");
}

#[test]
fn test_matched_string_tokenize_outer_text_ignored() {
    let tokens = matched_string_tokenize("out{in}out", '{', '}', '\0').unwrap();
    assert_eq!(tokens.len(), 1);
    assert_eq!(join(&tokens, " "), "in");
}

#[test]
fn test_matched_string_tokenize_empty_braces_and_nested() {
    let tokens = matched_string_tokenize("{} {} {stuff_{foo}_{bar}}", '{', '}', '\0').unwrap();
    assert_eq!(tokens.len(), 3);
    assert_eq!(join(&tokens, " "), "  stuff_{foo}_{bar}");
}

#[test]
fn test_matched_string_tokenize_deeply_nested() {
    let tokens = matched_string_tokenize("{and} {more{nested{braces}}}", '{', '}', '\0').unwrap();
    assert_eq!(tokens.len(), 2);
    assert_eq!(join(&tokens, " "), "and more{nested{braces}}");
}

// ---------------------------------------------------------------------------
// TestGetXmlEscapedString
// ---------------------------------------------------------------------------

#[test]
fn test_xml_escaped_string() {
    assert_eq!(get_xml_escaped_string("Amiga"), "Amiga");
    assert_eq!(get_xml_escaped_string("Amiga & Atari"), "Amiga &amp; Atari");
    assert_eq!(get_xml_escaped_string("Amiga < Atari"), "Amiga &lt; Atari");
    assert_eq!(get_xml_escaped_string("Amiga > Atari"), "Amiga &gt; Atari");
    assert_eq!(get_xml_escaped_string("\"Atari\""), "&quot;Atari&quot;");
    assert_eq!(get_xml_escaped_string("'Atari'"), "&apos;Atari&apos;");
}
