//! Character classification utilities for the USDA lexer.
//!
//! This module provides functions to classify characters according to
//! USD text file format rules. These are based on the PEGTL character
//! classes from `textFileFormatParser.h`.
//!
//! # Unicode Support
//!
//! USD identifiers support Unicode characters following the Unicode
//! XID (Identifier) properties:
//! - `XID_Start` - Characters that can start an identifier
//! - `XID_Continue` - Characters that can continue an identifier
//!
//! # ASCII Fast Paths
//!
//! For performance, ASCII characters have fast-path checks before
//! falling back to Unicode property lookups.

// ============================================================================
// Basic Character Classes
// ============================================================================

/// Returns true if the byte is an ASCII digit (0-9).
#[inline]
#[must_use]
pub const fn is_digit(b: u8) -> bool {
    b.is_ascii_digit()
}

/// Returns true if the byte is a hexadecimal digit (0-9, a-f, A-F).
#[inline]
#[must_use]
pub const fn is_hex_digit(b: u8) -> bool {
    b.is_ascii_hexdigit()
}

/// Returns true if the byte is an ASCII letter (a-z, A-Z).
#[inline]
#[must_use]
pub const fn is_alpha(b: u8) -> bool {
    b.is_ascii_alphabetic()
}

/// Returns true if the byte is ASCII alphanumeric (a-z, A-Z, 0-9).
#[inline]
#[must_use]
pub const fn is_alphanumeric(b: u8) -> bool {
    b.is_ascii_alphanumeric()
}

// ============================================================================
// Whitespace
// ============================================================================

/// Returns true if the byte is inline whitespace (space or tab).
///
/// This corresponds to the `Space` rule in C++ PEGTL.
#[inline]
#[must_use]
pub const fn is_inline_space(b: u8) -> bool {
    matches!(b, b' ' | b'\t')
}

/// Returns true if the byte is a newline character.
///
/// This corresponds to the `Eol` rule in C++ PEGTL.
#[inline]
#[must_use]
pub const fn is_newline(b: u8) -> bool {
    matches!(b, b'\n' | b'\r')
}

/// Returns true if the byte is any whitespace character.
#[inline]
#[must_use]
pub const fn is_whitespace(b: u8) -> bool {
    matches!(b, b' ' | b'\t' | b'\n' | b'\r')
}

// ============================================================================
// Identifier Characters
// ============================================================================

/// Returns true if the character can start a USD identifier.
///
/// USD identifiers follow Unicode XID_Start property, which includes:
/// - ASCII letters (a-z, A-Z)
/// - Underscore (_)
/// - Unicode letters with XID_Start property
///
/// # C++ Parity
///
/// This corresponds to `Utf8Identifier` start in the PEGTL grammar,
/// which uses Unicode identifier properties.
#[must_use]
pub fn is_identifier_start(ch: char) -> bool {
    // Fast path for ASCII
    if ch.is_ascii() {
        return ch.is_ascii_alphabetic() || ch == '_';
    }

    // Unicode XID_Start check
    // In full implementation, we'd use unicode-xid crate
    // For now, accept letters from Unicode general categories L*
    ch.is_alphabetic()
}

/// Returns true if the character can continue a USD identifier.
///
/// USD identifiers follow Unicode XID_Continue property, which includes:
/// - Everything in XID_Start
/// - ASCII digits (0-9)
/// - Unicode digits and combining marks
///
/// # C++ Parity
///
/// This corresponds to `Utf8Identifier` continuation in the PEGTL grammar.
#[must_use]
pub fn is_identifier_continue(ch: char) -> bool {
    // Fast path for ASCII
    if ch.is_ascii() {
        return ch.is_ascii_alphanumeric() || ch == '_';
    }

    // Unicode XID_Continue check
    ch.is_alphanumeric() || ch == '_'
}

/// Returns true if the byte can start an ASCII identifier.
///
/// This is a fast-path check for the common case.
#[inline]
#[must_use]
pub const fn is_ascii_identifier_start(b: u8) -> bool {
    b.is_ascii_alphabetic() || b == b'_'
}

/// Returns true if the byte can continue an ASCII identifier.
#[inline]
#[must_use]
pub const fn is_ascii_identifier_continue(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

// ============================================================================
// Number Characters
// ============================================================================

/// Returns true if the byte can start a number.
///
/// Numbers can start with:
/// - Digit (0-9)
/// - Minus sign (-)
/// - Dot followed by digit (.5)
#[inline]
#[must_use]
pub const fn is_number_start(b: u8) -> bool {
    is_digit(b) || b == b'-' || b == b'.'
}

/// Returns true if the byte is part of a number.
#[inline]
#[must_use]
pub const fn is_number_continue(b: u8) -> bool {
    is_digit(b) || b == b'.' || b == b'e' || b == b'E' || b == b'+' || b == b'-'
}

/// Returns true if the byte is an exponent marker (e or E).
#[inline]
#[must_use]
pub const fn is_exponent(b: u8) -> bool {
    matches!(b, b'e' | b'E')
}

/// Returns true if the byte is a sign (+/-).
#[inline]
#[must_use]
pub const fn is_sign(b: u8) -> bool {
    matches!(b, b'+' | b'-')
}

// ============================================================================
// Special Characters
// ============================================================================

/// Returns true if the byte is a quote character.
#[inline]
#[must_use]
pub const fn is_quote(b: u8) -> bool {
    matches!(b, b'"' | b'\'')
}

/// Returns true if the byte is an opening bracket.
#[inline]
#[must_use]
pub const fn is_opening_bracket(b: u8) -> bool {
    matches!(b, b'(' | b'[' | b'{' | b'<')
}

/// Returns true if the byte is a closing bracket.
#[inline]
#[must_use]
pub const fn is_closing_bracket(b: u8) -> bool {
    matches!(b, b')' | b']' | b'}' | b'>')
}

/// Returns true if the byte is a punctuation character.
#[inline]
#[must_use]
pub const fn is_punctuation(b: u8) -> bool {
    matches!(
        b,
        b'(' | b')'
            | b'['
            | b']'
            | b'{'
            | b'}'
            | b'<'
            | b'>'
            | b'='
            | b':'
            | b','
            | b';'
            | b'.'
            | b'&'
    )
}

// ============================================================================
// Escape Sequences
// ============================================================================

/// Returns the escaped character value for an escape sequence.
///
/// # Arguments
///
/// * `ch` - The character after the backslash
///
/// # Returns
///
/// The actual character value, or `None` if invalid.
#[must_use]
pub fn escape_char(ch: char) -> Option<char> {
    match ch {
        'n' => Some('\n'),
        'r' => Some('\r'),
        't' => Some('\t'),
        '\\' => Some('\\'),
        '"' => Some('"'),
        '\'' => Some('\''),
        '0' => Some('\0'),
        // Hex escape \xNN is handled separately
        _ => None,
    }
}

/// Returns true if the character is a valid escape sequence starter.
#[inline]
#[must_use]
pub const fn is_escape_char(ch: char) -> bool {
    matches!(ch, 'n' | 'r' | 't' | '\\' | '"' | '\'' | '0' | 'x')
}

// ============================================================================
// Path Characters
// ============================================================================

/// Returns true if the character is valid in a USD path.
///
/// Path characters include:
/// - Identifier characters
/// - Forward slash (/)
/// - Dot (.)
/// - Brackets ([ ])
/// - Colon (:)
#[must_use]
pub fn is_path_char(ch: char) -> bool {
    is_identifier_continue(ch) || matches!(ch, '/' | '.' | '[' | ']' | ':' | '{' | '}')
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_digit() {
        assert!(is_digit(b'0'));
        assert!(is_digit(b'9'));
        assert!(!is_digit(b'a'));
    }

    #[test]
    fn test_hex_digit() {
        assert!(is_hex_digit(b'0'));
        assert!(is_hex_digit(b'f'));
        assert!(is_hex_digit(b'F'));
        assert!(!is_hex_digit(b'g'));
    }

    #[test]
    fn test_whitespace() {
        assert!(is_inline_space(b' '));
        assert!(is_inline_space(b'\t'));
        assert!(!is_inline_space(b'\n'));
        assert!(is_newline(b'\n'));
        assert!(is_newline(b'\r'));
    }

    #[test]
    fn test_identifier() {
        assert!(is_identifier_start('a'));
        assert!(is_identifier_start('_'));
        assert!(!is_identifier_start('0'));

        assert!(is_identifier_continue('a'));
        assert!(is_identifier_continue('0'));
        assert!(is_identifier_continue('_'));
        assert!(!is_identifier_continue('-'));
    }

    #[test]
    fn test_number() {
        assert!(is_number_start(b'0'));
        assert!(is_number_start(b'-'));
        assert!(is_number_start(b'.'));
        assert!(!is_number_start(b'a'));

        assert!(is_exponent(b'e'));
        assert!(is_exponent(b'E'));
        assert!(!is_exponent(b'x'));
    }

    #[test]
    fn test_escape() {
        assert_eq!(escape_char('n'), Some('\n'));
        assert_eq!(escape_char('t'), Some('\t'));
        assert_eq!(escape_char('\\'), Some('\\'));
        assert_eq!(escape_char('z'), None);
    }

    #[test]
    fn test_punctuation() {
        assert!(is_punctuation(b'('));
        assert!(is_punctuation(b')'));
        assert!(is_punctuation(b'='));
        assert!(!is_punctuation(b'a'));
    }
}
