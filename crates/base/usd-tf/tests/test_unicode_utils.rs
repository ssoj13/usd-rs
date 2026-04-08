#![allow(unsafe_code)]
// Port of pxr/base/tf/testenv/unicodeUtils.cpp
// Tests for TfUnicodeUtils — code points, view/iterator, character classes,
// reflection, surrogate range, and dictionary ordering.

use usd_tf::string_utils::dictionary_less_than;
use usd_tf::unicode_utils::{
    MAXIMUM_VALUE, SURROGATE_END, SURROGATE_START, UTF8_INVALID_CODE_POINT, Utf8CodePoint,
    Utf8CodePointIterator, Utf8CodePointView, is_utf8_code_point_xid_continue,
    is_utf8_code_point_xid_start, is_xid_continue, is_xid_start,
};

// ---------------------------------------------------------------------------
// TestUtf8CodePoint
// ---------------------------------------------------------------------------

#[test]
fn test_code_point_default_is_replacement() {
    assert_eq!(Utf8CodePoint::default(), UTF8_INVALID_CODE_POINT);
}

#[test]
fn test_code_point_boundary_zero() {
    assert_eq!(Utf8CodePoint::new(0).as_u32(), 0);
}

#[test]
fn test_code_point_boundary_maximum() {
    assert_eq!(Utf8CodePoint::new(MAXIMUM_VALUE).as_u32(), MAXIMUM_VALUE);
}

#[test]
fn test_code_point_boundary_over_maximum() {
    assert_eq!(
        Utf8CodePoint::new(MAXIMUM_VALUE + 1),
        UTF8_INVALID_CODE_POINT
    );
}

#[test]
fn test_code_point_u32_max_is_invalid() {
    assert_eq!(Utf8CodePoint::new(u32::MAX), UTF8_INVALID_CODE_POINT);
}

#[test]
fn test_code_point_before_surrogate_range() {
    // One below SURROGATE_START is valid
    assert_eq!(
        Utf8CodePoint::new(SURROGATE_START - 1).as_u32(),
        SURROGATE_START - 1
    );
}

#[test]
fn test_code_point_after_surrogate_range() {
    // One above SURROGATE_END is valid
    assert_eq!(
        Utf8CodePoint::new(SURROGATE_END + 1).as_u32(),
        SURROGATE_END + 1
    );
}

#[test]
fn test_code_point_surrogate_start_is_invalid() {
    assert_eq!(Utf8CodePoint::new(SURROGATE_START), UTF8_INVALID_CODE_POINT);
}

#[test]
fn test_code_point_surrogate_end_is_invalid() {
    assert_eq!(Utf8CodePoint::new(SURROGATE_END), UTF8_INVALID_CODE_POINT);
}

#[test]
fn test_code_point_surrogate_midpoint_is_invalid() {
    let mid = (SURROGATE_START + SURROGATE_END) / 2;
    assert_eq!(Utf8CodePoint::new(mid), UTF8_INVALID_CODE_POINT);
}

#[test]
fn test_code_point_stringify_ascii() {
    // U+0061 = 'a'
    assert_eq!(format!("{}", Utf8CodePoint::new(97)), "a");
}

#[test]
fn test_code_point_stringify_integral() {
    // U+222B = ∫
    assert_eq!(format!("{}", Utf8CodePoint::new(8747)), "∫");
}

#[test]
fn test_code_point_stringify_invalid_is_replacement_char() {
    // Both the explicit invalid constant and default() must produce U+FFFD.
    let replacement_str = format!("{}", UTF8_INVALID_CODE_POINT);
    assert_eq!(format!("{}", Utf8CodePoint::default()), replacement_str);
    assert_eq!(replacement_str, "\u{FFFD}");
}

#[test]
fn test_code_point_from_ascii() {
    // from_ascii('a') == Utf8CodePoint(97)
    assert_eq!(Utf8CodePoint::from_ascii(b'a'), Utf8CodePoint::new(97));
    assert_eq!(format!("{}", Utf8CodePoint::from_ascii(b'a')), "a");
    // Byte >= 128 is not ASCII → replacement
    assert_eq!(Utf8CodePoint::from_ascii(128), UTF8_INVALID_CODE_POINT);
}

// ---------------------------------------------------------------------------
// TestUtf8CodePointView — iterator behaviour
// ---------------------------------------------------------------------------

#[test]
fn test_view_empty() {
    let view = Utf8CodePointView::new("");
    assert!(view.is_empty());
    assert_eq!(view.iter().count(), 0);
}

#[test]
fn test_view_s1_length_and_first_code_point() {
    // "ⅈ75_hgòð㤻" — 9 code points, first is U+2148 = ⅈ = 8520
    let s1 = "ⅈ75_hgòð㤻";
    let view = Utf8CodePointView::new(s1);
    let cps: Vec<_> = view.iter().collect();
    assert_eq!(cps.len(), 9);
    assert_ne!(cps[0], UTF8_INVALID_CODE_POINT);
    assert_eq!(cps[0], Utf8CodePoint::new(8520));
    // All must be valid
    for cp in &cps {
        assert_ne!(*cp, UTF8_INVALID_CODE_POINT);
    }
}

#[test]
fn test_view_s2_length_and_first_code_point() {
    // "㤼01৪∫" — 5 code points, first is U+39BC = 㤼 = 14652
    let s2 = "㤼01৪∫";
    let view = Utf8CodePointView::new(s2);
    let cps: Vec<_> = view.iter().collect();
    assert_eq!(cps.len(), 5);
    assert_ne!(cps[0], UTF8_INVALID_CODE_POINT);
    assert_eq!(cps[0], Utf8CodePoint::new(14652));
    for cp in &cps {
        assert_ne!(*cp, UTF8_INVALID_CODE_POINT);
    }
}

#[test]
fn test_view_s3_split_at_dash() {
    // "㤻üaf-∫⁇…🔗" — split on '-'
    let s3 = "㤻üaf-∫⁇…🔗";
    let view = Utf8CodePointView::new(s3);

    // First code point is U+39BB = 㤻 = 14651
    let cps: Vec<_> = view.iter().collect();
    assert_ne!(cps[0], UTF8_INVALID_CODE_POINT);
    assert_eq!(cps[0], Utf8CodePoint::new(14651));

    // 4 code points before '-', then '-' itself
    let dash_pos = s3.find('-').unwrap();
    let before_dash: Vec<_> = Utf8CodePointIterator::new(&s3[..dash_pos]).collect();
    assert_eq!(before_dash.len(), 4);

    let from_dash: Vec<_> = Utf8CodePointIterator::new(&s3[dash_pos..]).collect();
    // '-' is the first character from this sub-iterator
    assert_eq!(from_dash[0], Utf8CodePoint::from_ascii(b'-'));

    // All characters in the full string must be valid
    for cp in &cps {
        assert_ne!(*cp, UTF8_INVALID_CODE_POINT);
    }
}

#[test]
fn test_view_unexpected_continuation_bytes() {
    // \x80 and \x81 are continuation bytes with no leading byte → invalid.
    // Build the byte sequence dynamically to avoid the compile-time
    // invalid_from_utf8_unchecked lint on literal byte strings.
    let bytes: Vec<u8> = vec![0x80, 0x61, 0x62, 0x81, 0x63];
    // Safety: we intentionally pass invalid UTF-8 to test the decoder's
    // error-handling path. No string operations that assume valid UTF-8
    // are performed — the iterator only reads raw bytes.
    let sv = unsafe { std::str::from_utf8_unchecked(&bytes) };
    let view = Utf8CodePointView::new(sv);
    let cps: Vec<_> = view.iter().collect();
    // 5 code points total: invalid, 'a', 'b', invalid, 'c'
    assert_eq!(cps.len(), 5);
    assert_eq!(cps[0], UTF8_INVALID_CODE_POINT);
    assert_eq!(cps[1], Utf8CodePoint::from_ascii(b'a'));
    assert_eq!(cps[2], Utf8CodePoint::from_ascii(b'b'));
    assert_eq!(cps[3], UTF8_INVALID_CODE_POINT);
    assert_eq!(cps[4], Utf8CodePoint::from_ascii(b'c'));
}

#[test]
fn test_view_incomplete_sequences_do_not_consume_valid_chars() {
    // Incomplete leading bytes must not swallow the next valid ASCII byte.
    // Bytes: 0xC0 'a' 0xE0 0x85 'b' 0xF0 0x83 0x84 'c' 0xF1
    // Expected: invalid,'a',invalid,'b',invalid,'c',invalid — 7 code points.
    // Build dynamically to avoid the compile-time invalid_from_utf8_unchecked lint.
    let bytes: Vec<u8> = vec![0xC0, 0x61, 0xE0, 0x85, 0x62, 0xF0, 0x83, 0x84, 0x63, 0xF1];
    // Safety: intentionally invalid UTF-8 to test the error-handling path.
    let sv = unsafe { std::str::from_utf8_unchecked(&bytes) };
    let view = Utf8CodePointView::new(sv);
    let cps: Vec<_> = view.iter().collect();
    assert_eq!(cps.len(), 7);
    let expected = [
        UTF8_INVALID_CODE_POINT,
        Utf8CodePoint::from_ascii(b'a'),
        UTF8_INVALID_CODE_POINT,
        Utf8CodePoint::from_ascii(b'b'),
        UTF8_INVALID_CODE_POINT,
        Utf8CodePoint::from_ascii(b'c'),
        UTF8_INVALID_CODE_POINT,
    ];
    assert_eq!(cps, expected);
}

// ---------------------------------------------------------------------------
// TestCharacterClasses — XID_Start / XID_Continue
// ---------------------------------------------------------------------------

const XID_START_CODE_POINTS: &[u32] = &[
    0x0043,  // Latin capital letter C (Lu)
    0x006A,  // Latin small letter j (Ll)
    0x0254,  // Latin small letter open o (Ll)
    0x01C6,  // Latin small letter DZ with Caron (Ll)
    0x01CB,  // Latin capital letter N with small letter j (Lt)
    0x02B3,  // Modifier letter small r (Lm)
    0x10464, // Shavian letter Loll (Lo)
    0x132B5, // Egyptian hieroglyph R0004 (Lo)
    0x12421, // Cuneiform numeric sign four geshu (Nl)
    0xFDAB,  // Arabic Ligature (Lo)
    0x18966, // Tangut Component-359 (Lo)
    0x10144, // Greek acrophonic attic fifty (Nl)
    0x037F,  // Greek capital letter YOT (Lu) — singular range
    0x2F800, // CJK Compatibility Ideograph-2F800 (Lo) — start range
    0x3134A, // CJK Ideograph Extension G Last (Lo) — end range
];

const XID_CONTINUE_CODE_POINTS: &[u32] = &[
    0x0032,  // Digit two (Nd)
    0x0668,  // Arabic-Indic Digit Eight (Nd)
    0x07C0,  // NKo Digit Zero (Nd)
    0x1E145, // Nyiakeng Puachue Hmong Digit Five (Nd)
    0x0300,  // Combining Grave Accent (Mn)
    0x2CEF,  // Coptic Combining NI Above (Mn)
    0x10A02, // Kharoshthi Vowel Sign U (Mn)
    0x16F92, // Miao Tone Below (Mn)
    0x0903,  // Devanagari Sign Visarga (Mc)
    0x16F55, // Miao Vowel Sign AA (Mc)
    0x1D172, // Musical Symbol Combining Flag-5 (Mc)
    0x203F,  // Undertie (Pc)
    0x005F,  // Low line (underscore) (Pc)
    0xFE4F,  // Wavy Low Line (Pc)
    0x05BF,  // Hebrew Point Rafe (Mn) — singular range
    0x1E2EC, // Wancho Tone Tup (Mn) — start range
    0xE01EF, // Variation Selector-256 (Mn) — end range
];

const INVALID_CODE_POINTS: &[u32] = &[
    0x002D,  // Hyphen-Minus (Pd)
    0x00AB,  // Left-Pointing Double Angle Quotation Mark (Pi)
    0x2019,  // Right Single Quotation Mark (Pf)
    0x2021,  // Double Dagger (Po)
    0x1ECB0, // Indic Siyaq Rupee Mark (Sc)
    0x0020,  // Space (Zs)
    0x3000,  // Ideographic Space (Zs)
    0x000B,  // Line tabulation (Cc)
    0xF8FE,  // Private Use (Co)
];

#[test]
fn test_xid_start_code_points() {
    for &cp in XID_START_CODE_POINTS {
        let utf_cp = Utf8CodePoint::new(cp);
        assert!(
            is_utf8_code_point_xid_start(utf_cp),
            "Expected XID_Start for U+{cp:04X}"
        );
        // XID_Continue is a superset of XID_Start
        assert!(
            is_utf8_code_point_xid_continue(utf_cp),
            "Expected XID_Continue for U+{cp:04X}"
        );
    }
}

#[test]
fn test_xid_continue_only_code_points() {
    for &cp in XID_CONTINUE_CODE_POINTS {
        let utf_cp = Utf8CodePoint::new(cp);
        assert!(
            is_utf8_code_point_xid_continue(utf_cp),
            "Expected XID_Continue for U+{cp:04X}"
        );
    }
}

#[test]
fn test_invalid_code_points_not_xid() {
    for &cp in INVALID_CODE_POINTS {
        let utf_cp = Utf8CodePoint::new(cp);
        assert!(
            !is_utf8_code_point_xid_start(utf_cp),
            "Expected NOT XID_Start for U+{cp:04X}"
        );
        assert!(
            !is_utf8_code_point_xid_continue(utf_cp),
            "Expected NOT XID_Continue for U+{cp:04X}"
        );
    }
}

#[test]
fn test_xid_uint32_max_is_invalid() {
    assert!(!is_xid_start(u32::MAX));
    assert!(!is_xid_continue(u32::MAX));
}

#[test]
fn test_xid_maximum_value_is_invalid() {
    // U+10FFFF is not in XID_Start or XID_Continue
    assert!(!is_xid_start(MAXIMUM_VALUE));
    assert!(!is_xid_continue(MAXIMUM_VALUE));
    assert!(!is_xid_start(MAXIMUM_VALUE + 1));
    assert!(!is_xid_continue(MAXIMUM_VALUE + 1));
}

#[test]
fn test_character_class_string_s1() {
    // "ⅈ75_hgòð㤻": first is XID_Start, rest are XID_Continue
    let s1 = "ⅈ75_hgòð㤻";
    let view = Utf8CodePointView::new(s1);
    let cps: Vec<_> = view.iter().collect();
    assert_eq!(cps.len(), 9);
    assert!(
        is_utf8_code_point_xid_start(cps[0]),
        "First code point should be XID_Start"
    );
    for cp in &cps[1..] {
        assert!(
            is_utf8_code_point_xid_continue(*cp),
            "Remaining code points should be XID_Continue"
        );
    }
}

#[test]
fn test_character_class_string_s2() {
    // "㤼01৪∫": XID_Start, XID_Continue×3, then not XID_Continue (∫)
    let s2 = "㤼01৪∫";
    let view = Utf8CodePointView::new(s2);
    let cps: Vec<_> = view.iter().collect();
    assert_eq!(cps.len(), 5);
    assert!(is_utf8_code_point_xid_start(cps[0]));
    assert!(is_utf8_code_point_xid_continue(cps[1]));
    assert!(is_utf8_code_point_xid_continue(cps[2]));
    assert!(is_utf8_code_point_xid_continue(cps[3]));
    // ∫ (U+222B) is NOT XID_Continue
    assert!(!is_utf8_code_point_xid_continue(cps[4]));
}

#[test]
fn test_character_class_string_s3() {
    // "㤻üaf-∫⁇…🔗": split on '-'
    // Before '-': all XID_Start; after '-': none are XID_Continue
    let s3 = "㤻üaf-∫⁇…🔗";
    let dash_pos = s3.find('-').unwrap();
    let before = &s3[..dash_pos];
    let after = &s3[dash_pos..];

    for cp in Utf8CodePointIterator::new(before) {
        assert!(
            is_utf8_code_point_xid_start(cp),
            "Expected XID_Start before '-'"
        );
    }
    for cp in Utf8CodePointIterator::new(after) {
        assert!(
            !is_utf8_code_point_xid_continue(cp),
            "Expected NOT XID_Continue after (and including) '-'"
        );
    }
}

// ---------------------------------------------------------------------------
// TestUtf8CodePointReflection — every valid code point roundtrips
// ---------------------------------------------------------------------------

#[test]
fn test_code_point_reflection() {
    // Iterate over the full Unicode scalar value space.
    // Skip the surrogate range as those are not valid code points.
    for value in 0..=MAXIMUM_VALUE {
        if value >= SURROGATE_START && value <= SURROGATE_END {
            continue;
        }
        let cp = Utf8CodePoint::new(value);
        assert_eq!(
            cp.as_u32(),
            value,
            "Code point value mismatch at U+{value:04X}"
        );

        // Serialize to a UTF-8 string and read back
        let text = format!("{cp}");
        let view = Utf8CodePointView::new(&text);
        let mut iter = view.iter();
        let first = iter.next().expect("Expected at least one code point");
        assert_eq!(first, cp, "Reflection failed for U+{value:04X}");
        assert!(
            iter.next().is_none(),
            "Expected exactly one code point for U+{value:04X}"
        );
    }
}

// ---------------------------------------------------------------------------
// TestUtf8CodePointSurrogateRange — surrogates become replacement
// ---------------------------------------------------------------------------

#[test]
fn test_surrogate_range_all_become_invalid() {
    for value in SURROGATE_START..=SURROGATE_END {
        let cp = Utf8CodePoint::new(value);
        assert_eq!(
            cp, UTF8_INVALID_CODE_POINT,
            "Surrogate U+{value:04X} should equal invalid code point"
        );
        assert_eq!(
            format!("{cp}"),
            format!("{UTF8_INVALID_CODE_POINT}"),
            "Surrogate U+{value:04X} should stringify to replacement char"
        );
    }
}

// ---------------------------------------------------------------------------
// TestUtf8DictionaryLessThanOrdering — non-ASCII ordered by code point
// ---------------------------------------------------------------------------

#[test]
fn test_dict_less_than_ascii_all_less_than_first_non_ascii() {
    let non_ascii_str = format!("{}", Utf8CodePoint::new(128));
    for value in 0u32..=127 {
        let ascii_str = format!("{}", Utf8CodePoint::new(value));
        assert!(
            dictionary_less_than(&ascii_str, &non_ascii_str),
            "ASCII U+{value:04X} should be less than U+0080"
        );
    }
}

#[test]
fn test_dict_less_than_non_ascii_numerically_ordered() {
    // Non-ASCII code points must be in numerical order (skipping surrogates).
    for value in 129u32..=MAXIMUM_VALUE {
        if value >= SURROGATE_START && value <= SURROGATE_END + 1 {
            continue;
        }
        let current = format!("{}", Utf8CodePoint::new(value));
        let previous = format!("{}", Utf8CodePoint::new(value - 1));
        assert!(
            dictionary_less_than(&previous, &current),
            "U+{:04X} should be less than U+{value:04X}",
            value - 1
        );
    }
}

#[test]
fn test_dict_less_than_first_after_surrogate_greater_than_last_before() {
    // The first valid code point after the surrogate range must sort after the
    // last valid code point before it.
    let before = format!("{}", Utf8CodePoint::new(SURROGATE_START - 1));
    let after = format!("{}", Utf8CodePoint::new(SURROGATE_END + 1));
    assert!(dictionary_less_than(&before, &after));
}
