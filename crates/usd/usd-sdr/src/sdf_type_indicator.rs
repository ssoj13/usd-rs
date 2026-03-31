//! SDR SDF Type Indicator - Mapping between SDR property types and SDF types.
//!
//! Port of pxr/usd/sdr/sdfTypeIndicator.h
//!
//! This module provides SdrSdfTypeIndicator which represents a mapping from
//! an SDR property type to an SDF type. It handles both exact mappings
//! (where a direct SDF equivalent exists) and inexact mappings (where Token
//! is used as a fallback).
//!
//! Used by: SdrShaderProperty::get_type_as_sdf_type()
//! Uses: SdfValueTypeName, Token

use usd_sdf::ValueTypeName;
use usd_tf::Token;

/// Represents a mapping from an SDR property type to an SDF type.
///
/// If an exact mapping exists from SDR property type to SDF type, `has_sdf_type()`
/// will return true, and `get_sdf_type()` will return the SDF type resulting from
/// the conversion. `get_sdr_type()` will return the original SDR property type.
///
/// If a mapping doesn't exist from SDR property type to SDF type, `has_sdf_type()`
/// will return false, and `get_sdf_type()` will return either Token or TokenArray.
/// `get_sdr_type()` will return the original SDR property type.
#[derive(Debug, Clone)]
pub struct SdrSdfTypeIndicator {
    /// The SDF value type name.
    sdf_type: ValueTypeName,
    /// The original SDR property type.
    sdr_type: Token,
    /// Whether an exact mapping exists.
    has_sdf_type_mapping: bool,
}

impl SdrSdfTypeIndicator {
    /// Creates a default (empty) type indicator.
    pub fn new() -> Self {
        Self {
            sdf_type: ValueTypeName::default(),
            sdr_type: Token::default(),
            has_sdf_type_mapping: false,
        }
    }

    /// Creates a type indicator with the specified types.
    ///
    /// The `sdf_type` must be Token or TokenArray if `has_sdf_type_mapping` is false.
    pub fn with_types(
        sdf_type: ValueTypeName,
        sdr_type: Token,
        has_sdf_type_mapping: bool,
    ) -> Self {
        Self {
            sdf_type,
            sdr_type,
            has_sdf_type_mapping,
        }
    }

    /// Gets the original SDR property type associated with this mapping.
    pub fn get_sdr_type(&self) -> &Token {
        &self.sdr_type
    }

    /// Returns whether an exact SDF type exists for the represented SDR property type.
    pub fn has_sdf_type(&self) -> bool {
        self.has_sdf_type_mapping
    }

    /// Gets the SDF type associated with this mapping.
    ///
    /// If there is no valid SDF type, either Token or TokenArray is returned.
    pub fn get_sdf_type(&self) -> &ValueTypeName {
        &self.sdf_type
    }
}

impl Default for SdrSdfTypeIndicator {
    fn default() -> Self {
        Self::new()
    }
}

impl PartialEq for SdrSdfTypeIndicator {
    fn eq(&self, other: &Self) -> bool {
        // C++ compares only sdf_type and sdr_type, NOT has_sdf_type_mapping
        self.sdf_type == other.sdf_type && self.sdr_type == other.sdr_type
    }
}

impl Eq for SdrSdfTypeIndicator {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default() {
        let indicator = SdrSdfTypeIndicator::new();
        assert!(!indicator.has_sdf_type());
    }

    #[test]
    fn test_with_mapping() {
        let sdr_type = Token::new("float");
        let indicator =
            SdrSdfTypeIndicator::with_types(ValueTypeName::default(), sdr_type.clone(), true);
        assert!(indicator.has_sdf_type());
        assert_eq!(indicator.get_sdr_type(), &sdr_type);
    }

    #[test]
    fn test_without_mapping() {
        let sdr_type = Token::new("custom_type");
        let indicator =
            SdrSdfTypeIndicator::with_types(ValueTypeName::default(), sdr_type.clone(), false);
        assert!(!indicator.has_sdf_type());
        assert_eq!(indicator.get_sdr_type(), &sdr_type);
    }
}
