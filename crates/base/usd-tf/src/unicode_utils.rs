//! UTF-8 and Unicode utilities.
//!
//! Provides utilities for working with UTF-8 encoded strings and Unicode code points.
//!
//! # Examples
//!
//! ```
//! use usd_tf::unicode_utils::{Utf8CodePoint, Utf8CodePointView};
//!
//! let view = Utf8CodePointView::new("Hello ∫ 世界");
//! for cp in view {
//!     println!("Code point: U+{:04X}", cp.as_u32());
//! }
//! ```

use std::fmt;
use std::iter::FusedIterator;

/// Replacement code point value (U+FFFD).
pub const REPLACEMENT_VALUE: u32 = 0xFFFD;

/// Maximum valid code point value.
pub const MAXIMUM_VALUE: u32 = 0x10FFFF;

/// Start of surrogate range (invalid in UTF-8).
pub const SURROGATE_START: u32 = 0xD800;

/// End of surrogate range (invalid in UTF-8).
pub const SURROGATE_END: u32 = 0xDFFF;

/// Wrapper for a 32-bit Unicode code point value.
///
/// Code points outside the valid range or in the surrogate range
/// are replaced with the replacement character (U+FFFD).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Utf8CodePoint {
    value: u32,
}

impl Utf8CodePoint {
    /// The replacement code point (U+FFFD).
    pub const REPLACEMENT: Self = Self {
        value: REPLACEMENT_VALUE,
    };

    /// Maximum valid code point value.
    pub const MAX: Self = Self {
        value: MAXIMUM_VALUE,
    };

    /// Create a new code point from a u32 value.
    ///
    /// If the value is outside the valid range or in the surrogate range,
    /// it will be replaced with the replacement character.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_tf::unicode_utils::Utf8CodePoint;
    ///
    /// let cp = Utf8CodePoint::new(0x41); // 'A'
    /// assert_eq!(cp.as_u32(), 0x41);
    ///
    /// let invalid = Utf8CodePoint::new(0xD800); // Surrogate
    /// assert_eq!(invalid.as_u32(), 0xFFFD); // Replaced
    /// ```
    #[inline]
    pub const fn new(value: u32) -> Self {
        let is_valid = value <= MAXIMUM_VALUE && (value < SURROGATE_START || value > SURROGATE_END);

        Self {
            value: if is_valid { value } else { REPLACEMENT_VALUE },
        }
    }

    /// Create from a Rust char.
    ///
    /// This is always valid since Rust chars are valid Unicode scalar values.
    #[inline]
    pub const fn from_char(c: char) -> Self {
        Self { value: c as u32 }
    }

    /// Create from an ASCII character (0-127).
    ///
    /// Returns the replacement character if the byte is not valid ASCII.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_tf::unicode_utils::Utf8CodePoint;
    ///
    /// let cp = Utf8CodePoint::from_ascii(b'A');
    /// assert_eq!(cp.as_u32(), 0x41);
    ///
    /// let invalid = Utf8CodePoint::from_ascii(0x80);
    /// assert_eq!(invalid.as_u32(), 0xFFFD);
    /// ```
    #[inline]
    pub const fn from_ascii(byte: u8) -> Self {
        if byte < 128 {
            Self { value: byte as u32 }
        } else {
            Self::REPLACEMENT
        }
    }

    /// Returns the code point value as a u32.
    #[inline]
    pub const fn as_u32(self) -> u32 {
        self.value
    }

    /// Returns true if this is the replacement character.
    #[inline]
    pub const fn is_replacement(self) -> bool {
        self.value == REPLACEMENT_VALUE
    }

    /// Try to convert to a Rust char.
    ///
    /// Returns None if the code point is not a valid Unicode scalar value.
    #[inline]
    pub fn to_char(self) -> Option<char> {
        char::from_u32(self.value)
    }

    /// Returns true if this code point is valid.
    ///
    /// Note: The replacement character (U+FFFD) is itself a valid code point.
    #[inline]
    pub const fn is_valid(self) -> bool {
        // All stored values are valid since the constructor validates them
        true
    }
}

impl Default for Utf8CodePoint {
    fn default() -> Self {
        Self::REPLACEMENT
    }
}

impl From<char> for Utf8CodePoint {
    fn from(c: char) -> Self {
        Self::from_char(c)
    }
}

impl From<u32> for Utf8CodePoint {
    fn from(value: u32) -> Self {
        Self::new(value)
    }
}

impl fmt::Debug for Utf8CodePoint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "U+{:04X}", self.value)
    }
}

impl fmt::Display for Utf8CodePoint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(c) = self.to_char() {
            write!(f, "{}", c)
        } else {
            write!(f, "\u{FFFD}")
        }
    }
}

/// Invalid code point constant.
pub const UTF8_INVALID_CODE_POINT: Utf8CodePoint = Utf8CodePoint::REPLACEMENT;

/// Iterator over UTF-8 code points in a string.
#[derive(Clone)]
pub struct Utf8CodePointIterator<'a> {
    /// Remaining bytes.
    bytes: &'a [u8],
}

impl<'a> Utf8CodePointIterator<'a> {
    /// Create a new iterator over the given UTF-8 bytes.
    pub fn new(s: &'a str) -> Self {
        Self {
            bytes: s.as_bytes(),
        }
    }

    /// Returns true if the iterator has no more code points.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }
}

impl Iterator for Utf8CodePointIterator<'_> {
    type Item = Utf8CodePoint;

    fn next(&mut self) -> Option<Self::Item> {
        fn consumed_invalid_sequence_prefix(bytes: &[u8], expected_len: usize) -> usize {
            let mut consumed = 1usize;
            while consumed < expected_len
                && consumed < bytes.len()
                && (bytes[consumed] & 0xC0) == 0x80
            {
                consumed += 1;
            }
            consumed
        }

        if self.bytes.is_empty() {
            return None;
        }

        let first = self.bytes[0];

        // Determine encoding length and extract code point
        let (cp, len) = if first < 0x80 {
            // ASCII (1 byte)
            (first as u32, 1)
        } else if first < 0xC0 {
            // Invalid leading byte (continuation byte)
            (REPLACEMENT_VALUE, 1)
        } else if first < 0xE0 {
            // 2-byte sequence
            if self.bytes.len() < 2 {
                (
                    REPLACEMENT_VALUE,
                    consumed_invalid_sequence_prefix(self.bytes, 2),
                )
            } else {
                let b1 = self.bytes[1];
                if (b1 & 0xC0) != 0x80 {
                    (REPLACEMENT_VALUE, 1)
                } else {
                    let cp = ((first as u32 & 0x1F) << 6) | (b1 as u32 & 0x3F);
                    // Check for overlong encoding
                    if cp < 0x80 {
                        (REPLACEMENT_VALUE, 2)
                    } else {
                        (cp, 2)
                    }
                }
            }
        } else if first < 0xF0 {
            // 3-byte sequence
            if self.bytes.len() < 3 {
                (
                    REPLACEMENT_VALUE,
                    consumed_invalid_sequence_prefix(self.bytes, 3),
                )
            } else {
                let b1 = self.bytes[1];
                let b2 = self.bytes[2];
                if (b1 & 0xC0) != 0x80 {
                    (REPLACEMENT_VALUE, 1)
                } else if (b2 & 0xC0) != 0x80 {
                    (REPLACEMENT_VALUE, 2)
                } else {
                    let cp = ((first as u32 & 0x0F) << 12)
                        | ((b1 as u32 & 0x3F) << 6)
                        | (b2 as u32 & 0x3F);
                    // Check for overlong encoding and surrogates
                    if cp < 0x800 || (SURROGATE_START..=SURROGATE_END).contains(&cp) {
                        (REPLACEMENT_VALUE, 3)
                    } else {
                        (cp, 3)
                    }
                }
            }
        } else if first < 0xF8 {
            // 4-byte sequence
            if self.bytes.len() < 4 {
                (
                    REPLACEMENT_VALUE,
                    consumed_invalid_sequence_prefix(self.bytes, 4),
                )
            } else {
                let b1 = self.bytes[1];
                let b2 = self.bytes[2];
                let b3 = self.bytes[3];
                if (b1 & 0xC0) != 0x80 {
                    (REPLACEMENT_VALUE, 1)
                } else if (b2 & 0xC0) != 0x80 {
                    (REPLACEMENT_VALUE, 2)
                } else if (b3 & 0xC0) != 0x80 {
                    (REPLACEMENT_VALUE, 3)
                } else {
                    let cp = ((first as u32 & 0x07) << 18)
                        | ((b1 as u32 & 0x3F) << 12)
                        | ((b2 as u32 & 0x3F) << 6)
                        | (b3 as u32 & 0x3F);
                    // Check for overlong encoding and max value
                    if !(0x10000..=MAXIMUM_VALUE).contains(&cp) {
                        (REPLACEMENT_VALUE, 4)
                    } else {
                        (cp, 4)
                    }
                }
            }
        } else {
            // Invalid leading byte
            (REPLACEMENT_VALUE, 1)
        };

        self.bytes = &self.bytes[len..];
        Some(Utf8CodePoint::new(cp))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        // At least 1 code point per 4 bytes, at most 1 per byte
        let min = self.bytes.len().div_ceil(4);
        let max = self.bytes.len();
        (min, Some(max))
    }
}

impl FusedIterator for Utf8CodePointIterator<'_> {}

/// View over a UTF-8 string that iterates as code points.
#[derive(Clone, Copy)]
pub struct Utf8CodePointView<'a> {
    s: &'a str,
}

impl<'a> Utf8CodePointView<'a> {
    /// Create a new view over the given string.
    pub fn new(s: &'a str) -> Self {
        Self { s }
    }

    /// Returns true if the view is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.s.is_empty()
    }

    /// Returns an iterator over code points.
    #[inline]
    pub fn iter(&self) -> Utf8CodePointIterator<'a> {
        Utf8CodePointIterator::new(self.s)
    }

    /// Returns the underlying string slice.
    #[inline]
    pub fn as_str(&self) -> &'a str {
        self.s
    }
}

impl<'a> IntoIterator for Utf8CodePointView<'a> {
    type Item = Utf8CodePoint;
    type IntoIter = Utf8CodePointIterator<'a>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'a> IntoIterator for &'a Utf8CodePointView<'a> {
    type Item = Utf8CodePoint;
    type IntoIter = Utf8CodePointIterator<'a>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl fmt::Debug for Utf8CodePointView<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Utf8CodePointView")
            .field("s", &self.s)
            .finish()
    }
}

/// Check if a code point is in the XID_Start character class.
///
/// XID_Start includes uppercase/lowercase/titlecase/modifier/other letters
/// and letter numbers, suitable for the start of an identifier.
/// Uses `unicode-xid` crate for full Unicode Character Database compliance.
///
/// # Examples
///
/// ```
/// use usd_tf::unicode_utils::is_xid_start;
///
/// assert!(is_xid_start('A' as u32));
/// assert!(is_xid_start('a' as u32));
/// assert!(is_xid_start('_' as u32) == false); // underscore is not XID_Start
/// assert!(!is_xid_start('0' as u32));
/// ```
pub fn is_xid_start(code_point: u32) -> bool {
    if let Some(c) = char::from_u32(code_point) {
        unicode_xid::UnicodeXID::is_xid_start(c)
    } else {
        false
    }
}

/// Check if a Utf8CodePoint is in the XID_Start character class.
#[inline]
pub fn is_utf8_code_point_xid_start(cp: Utf8CodePoint) -> bool {
    is_xid_start(cp.as_u32())
}

/// Check if a code point is in the XID_Continue character class.
///
/// XID_Continue includes XID_Start plus nonspacing marks, spacing combining
/// marks, decimal numbers, and connector punctuation.
/// Uses `unicode-xid` crate for full Unicode Character Database compliance.
///
/// # Examples
///
/// ```
/// use usd_tf::unicode_utils::is_xid_continue;
///
/// assert!(is_xid_continue('A' as u32));
/// assert!(is_xid_continue('a' as u32));
/// assert!(is_xid_continue('0' as u32));
/// assert!(is_xid_continue('_' as u32));
/// ```
pub fn is_xid_continue(code_point: u32) -> bool {
    if let Some(c) = char::from_u32(code_point) {
        unicode_xid::UnicodeXID::is_xid_continue(c)
    } else {
        false
    }
}

/// Check if a Utf8CodePoint is in the XID_Continue character class.
#[inline]
pub fn is_utf8_code_point_xid_continue(cp: Utf8CodePoint) -> bool {
    is_xid_continue(cp.as_u32())
}

/// Count the number of UTF-8 code points in a string.
///
/// This is equivalent to string.chars().count() but is explicit about
/// counting code points.
pub fn count_code_points(s: &str) -> usize {
    s.chars().count()
}

/// Encode a code point as UTF-8 into a buffer.
///
/// Returns the number of bytes written (1-4), or 0 if the code point is invalid.
pub fn encode_utf8(code_point: u32, buffer: &mut [u8; 4]) -> usize {
    if let Some(c) = char::from_u32(code_point) {
        c.encode_utf8(buffer).len()
    } else {
        // Encode replacement character
        let replacement = '\u{FFFD}';
        replacement.encode_utf8(buffer).len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_code_point_new() {
        // Valid ASCII
        let cp = Utf8CodePoint::new(0x41);
        assert_eq!(cp.as_u32(), 0x41);
        assert!(!cp.is_replacement());

        // Valid Unicode
        let cp = Utf8CodePoint::new(0x222B); // ∫
        assert_eq!(cp.as_u32(), 0x222B);

        // Surrogate (invalid)
        let cp = Utf8CodePoint::new(0xD800);
        assert_eq!(cp.as_u32(), REPLACEMENT_VALUE);
        assert!(cp.is_replacement());

        // Over maximum
        let cp = Utf8CodePoint::new(0x200000);
        assert_eq!(cp.as_u32(), REPLACEMENT_VALUE);
    }

    #[test]
    fn test_code_point_from_char() {
        let cp = Utf8CodePoint::from_char('∫');
        assert_eq!(cp.as_u32(), 0x222B);
        assert_eq!(cp.to_char(), Some('∫'));
    }

    #[test]
    fn test_code_point_from_ascii() {
        let cp = Utf8CodePoint::from_ascii(b'A');
        assert_eq!(cp.as_u32(), 0x41);

        let cp = Utf8CodePoint::from_ascii(0x80);
        assert!(cp.is_replacement());
    }

    #[test]
    fn test_code_point_display() {
        let cp = Utf8CodePoint::new(0x41);
        assert_eq!(format!("{}", cp), "A");

        let cp = Utf8CodePoint::new(0x222B);
        assert_eq!(format!("{}", cp), "∫");
    }

    #[test]
    fn test_code_point_debug() {
        let cp = Utf8CodePoint::new(0x41);
        assert_eq!(format!("{:?}", cp), "U+0041");
    }

    #[test]
    fn test_iterator_ascii() {
        let view = Utf8CodePointView::new("ABC");
        let cps: Vec<_> = view.into_iter().collect();
        assert_eq!(cps.len(), 3);
        assert_eq!(cps[0].as_u32(), 0x41);
        assert_eq!(cps[1].as_u32(), 0x42);
        assert_eq!(cps[2].as_u32(), 0x43);
    }

    #[test]
    fn test_iterator_unicode() {
        let view = Utf8CodePointView::new("∫dx");
        let cps: Vec<_> = view.into_iter().collect();
        assert_eq!(cps.len(), 3);
        assert_eq!(cps[0].as_u32(), 0x222B); // ∫
        assert_eq!(cps[1].as_u32(), 0x64); // d
        assert_eq!(cps[2].as_u32(), 0x78); // x
    }

    #[test]
    fn test_iterator_mixed() {
        let view = Utf8CodePointView::new("Hello 世界");
        let cps: Vec<_> = view.into_iter().collect();
        assert_eq!(cps.len(), 8);
        assert_eq!(cps[0].as_u32(), 'H' as u32);
        assert_eq!(cps[6].as_u32(), '世' as u32);
        assert_eq!(cps[7].as_u32(), '界' as u32);
    }

    #[test]
    fn test_iterator_empty() {
        let view = Utf8CodePointView::new("");
        let cps: Vec<_> = view.into_iter().collect();
        assert!(cps.is_empty());
    }

    #[test]
    fn test_view_is_empty() {
        assert!(Utf8CodePointView::new("").is_empty());
        assert!(!Utf8CodePointView::new("a").is_empty());
    }

    #[test]
    fn test_view_as_str() {
        let view = Utf8CodePointView::new("hello");
        assert_eq!(view.as_str(), "hello");
    }

    #[test]
    fn test_xid_start() {
        assert!(is_xid_start('A' as u32));
        assert!(is_xid_start('a' as u32));
        assert!(is_xid_start('Z' as u32));
        assert!(!is_xid_start('0' as u32));
        assert!(!is_xid_start('_' as u32));
        assert!(!is_xid_start(' ' as u32));
    }

    #[test]
    fn test_xid_continue() {
        assert!(is_xid_continue('A' as u32));
        assert!(is_xid_continue('a' as u32));
        assert!(is_xid_continue('0' as u32));
        assert!(is_xid_continue('9' as u32));
        assert!(is_xid_continue('_' as u32));
        assert!(!is_xid_continue(' ' as u32));
        assert!(!is_xid_continue('-' as u32));
    }

    #[test]
    fn test_count_code_points() {
        assert_eq!(count_code_points("hello"), 5);
        assert_eq!(count_code_points("∫dx"), 3);
        assert_eq!(count_code_points("世界"), 2);
        assert_eq!(count_code_points(""), 0);
    }

    #[test]
    fn test_encode_utf8() {
        let mut buf = [0u8; 4];

        let len = encode_utf8('A' as u32, &mut buf);
        assert_eq!(len, 1);
        assert_eq!(buf[0], b'A');

        let len = encode_utf8('∫' as u32, &mut buf);
        assert_eq!(len, 3);

        let len = encode_utf8('😀' as u32, &mut buf);
        assert_eq!(len, 4);
    }

    #[test]
    fn test_encode_utf8_invalid() {
        let mut buf = [0u8; 4];

        // Invalid code point gets replacement
        let len = encode_utf8(0xD800, &mut buf);
        assert_eq!(len, 3); // Replacement is 3 bytes
    }

    #[test]
    fn test_iterator_4byte() {
        // Emoji requires 4 bytes in UTF-8
        let view = Utf8CodePointView::new("😀");
        let cps: Vec<_> = view.into_iter().collect();
        assert_eq!(cps.len(), 1);
        assert_eq!(cps[0].as_u32(), 0x1F600);
    }

    #[test]
    fn test_iterator_size_hint() {
        let iter = Utf8CodePointIterator::new("hello");
        let (min, max) = iter.size_hint();
        assert!(min <= 5);
        assert_eq!(max, Some(5));
    }

    #[test]
    fn test_code_point_eq() {
        let cp1 = Utf8CodePoint::new(0x41);
        let cp2 = Utf8CodePoint::new(0x41);
        let cp3 = Utf8CodePoint::new(0x42);

        assert_eq!(cp1, cp2);
        assert_ne!(cp1, cp3);
    }

    #[test]
    fn test_code_point_ord() {
        let cp_a = Utf8CodePoint::new(0x41);
        let cp_b = Utf8CodePoint::new(0x42);

        assert!(cp_a < cp_b);
    }

    #[test]
    fn test_code_point_from_u32() {
        let cp: Utf8CodePoint = 0x41u32.into();
        assert_eq!(cp.as_u32(), 0x41);
    }

    #[test]
    fn test_utf8_code_point_xid_functions() {
        let cp_a = Utf8CodePoint::new('A' as u32);
        let cp_0 = Utf8CodePoint::new('0' as u32);

        assert!(is_utf8_code_point_xid_start(cp_a));
        assert!(!is_utf8_code_point_xid_start(cp_0));
        assert!(is_utf8_code_point_xid_continue(cp_a));
        assert!(is_utf8_code_point_xid_continue(cp_0));
    }
}
