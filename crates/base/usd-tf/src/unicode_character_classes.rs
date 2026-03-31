//! Unicode character class data for XID_Start and XID_Continue.
//!
//! Port of pxr/base/tf/unicodeCharacterClasses.h
//!
//! Provides functions to query whether a Unicode code point belongs to
//! the XID_Start or XID_Continue character classes, used for identifier
//! validation. Delegates to the `unicode-xid` crate for actual tables.

use unicode_xid::UnicodeXID;

/// Maximum Unicode code point (17 planes * 2^16).
pub const MAX_CODE_POINT: u32 = 1_114_112;

/// Check if a code point is in the XID_Start character class.
///
/// XID_Start includes letters and underscore-like characters that can
/// begin an identifier.
///
/// # Examples
///
/// ```
/// use usd_tf::unicode_character_classes::is_xid_start;
///
/// assert!(is_xid_start('A' as u32));
/// assert!(is_xid_start('z' as u32));
/// assert!(!is_xid_start('0' as u32));
/// assert!(!is_xid_start(' ' as u32));
/// ```
pub fn is_xid_start(code_point: u32) -> bool {
    if code_point >= MAX_CODE_POINT {
        return false;
    }
    char::from_u32(code_point)
        .map(UnicodeXID::is_xid_start)
        .unwrap_or(false)
}

/// Check if a code point is in the XID_Continue character class.
///
/// XID_Continue includes characters that can appear after the first character
/// of an identifier (letters, digits, combining marks, connector punctuation).
///
/// # Examples
///
/// ```
/// use usd_tf::unicode_character_classes::is_xid_continue;
///
/// assert!(is_xid_continue('A' as u32));
/// assert!(is_xid_continue('0' as u32));
/// assert!(is_xid_continue('_' as u32));
/// assert!(!is_xid_continue(' ' as u32));
/// ```
pub fn is_xid_continue(code_point: u32) -> bool {
    if code_point >= MAX_CODE_POINT {
        return false;
    }
    char::from_u32(code_point)
        .map(UnicodeXID::is_xid_continue)
        .unwrap_or(false)
}

/// Query object for XID_Start character class.
///
/// Matches C++ `TfUnicodeXidStartFlagData`.
/// Uses the `unicode-xid` crate instead of a precomputed bitset.
pub struct XidStartFlagData;

impl XidStartFlagData {
    /// Check if the given code point is in XID_Start.
    pub fn is_xid_start_code_point(&self, code_point: u32) -> bool {
        is_xid_start(code_point)
    }
}

/// Query object for XID_Continue character class.
///
/// Matches C++ `TfUnicodeXidContinueFlagData`.
pub struct XidContinueFlagData;

impl XidContinueFlagData {
    /// Check if the given code point is in XID_Continue.
    pub fn is_xid_continue_code_point(&self, code_point: u32) -> bool {
        is_xid_continue(code_point)
    }
}

/// Get a reference to the XID_Start flag data.
///
/// Matches C++ `TfUnicodeGetXidStartFlagData()`.
pub fn get_xid_start_flag_data() -> XidStartFlagData {
    XidStartFlagData
}

/// Get a reference to the XID_Continue flag data.
///
/// Matches C++ `TfUnicodeGetXidContinueFlagData()`.
pub fn get_xid_continue_flag_data() -> XidContinueFlagData {
    XidContinueFlagData
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_xid_start() {
        // ASCII letters
        assert!(is_xid_start('A' as u32));
        assert!(is_xid_start('Z' as u32));
        assert!(is_xid_start('a' as u32));
        assert!(is_xid_start('z' as u32));

        // Underscore is NOT XID_Start (it's Pc = Connector Punctuation, XID_Continue only)
        assert!(!is_xid_start('_' as u32));

        // Digits are NOT XID_Start
        assert!(!is_xid_start('0' as u32));
        assert!(!is_xid_start('9' as u32));

        // Space and punctuation are not XID_Start
        assert!(!is_xid_start(' ' as u32));
        assert!(!is_xid_start('!' as u32));
    }

    #[test]
    fn test_xid_continue() {
        // Letters
        assert!(is_xid_continue('A' as u32));
        assert!(is_xid_continue('z' as u32));

        // Digits ARE XID_Continue
        assert!(is_xid_continue('0' as u32));
        assert!(is_xid_continue('9' as u32));

        // Underscore
        assert!(is_xid_continue('_' as u32));

        // Space is not XID_Continue
        assert!(!is_xid_continue(' ' as u32));
    }

    #[test]
    fn test_unicode_letters() {
        // CJK character (U+4E00)
        assert!(is_xid_start(0x4E00));
        assert!(is_xid_continue(0x4E00));

        // Cyrillic letter (U+0410 = A)
        assert!(is_xid_start(0x0410));
    }

    #[test]
    fn test_out_of_range() {
        assert!(!is_xid_start(MAX_CODE_POINT));
        assert!(!is_xid_continue(MAX_CODE_POINT));
        assert!(!is_xid_start(u32::MAX));
        assert!(!is_xid_continue(u32::MAX));
    }

    #[test]
    fn test_surrogate_range() {
        // Surrogates (U+D800-U+DFFF) are not valid code points
        assert!(!is_xid_start(0xD800));
        assert!(!is_xid_continue(0xD800));
    }

    #[test]
    fn test_flag_data_objects() {
        let start_data = get_xid_start_flag_data();
        let cont_data = get_xid_continue_flag_data();

        assert!(start_data.is_xid_start_code_point('A' as u32));
        assert!(cont_data.is_xid_continue_code_point('0' as u32));
    }
}
