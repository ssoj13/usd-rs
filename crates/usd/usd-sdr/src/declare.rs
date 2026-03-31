//! SDR Declarations - Common type definitions for the Shader Definition Registry.
//!
//! Port of pxr/usd/sdr/declare.h
//!
//! This module provides:
//! - Type aliases for identifiers, tokens, and collections
//! - SdrVersion struct for shader version management
//! - SdrVersionFilter enum for version filtering
//!
//! These types are used throughout SDR for shader node identification and versioning.

use std::collections::{HashMap, HashSet};
use std::fmt;
use std::hash::{Hash, Hasher};
use usd_tf::Token;

// ============================================================================
// Type Aliases
// ============================================================================

/// Shader identifier (same as Token).
pub type SdrIdentifier = Token;

/// Vector of identifiers.
pub type SdrIdentifierVec = Vec<SdrIdentifier>;

/// Set of identifiers.
pub type SdrIdentifierSet = HashSet<SdrIdentifier>;

/// Vector of tokens.
pub type SdrTokenVec = Vec<Token>;

/// Map from token to string.
pub type SdrTokenMap = HashMap<Token, String>;

/// Vector of strings.
pub type SdrStringVec = Vec<String>;

/// Set of strings.
pub type SdrStringSet = HashSet<String>;

/// Returns the string representation of the given shader identifier.
///
/// Matches C++ `SdrGetIdentifierString(const SdrIdentifier&)` from declare.h.
#[inline]
pub fn sdr_get_identifier_string(id: &SdrIdentifier) -> &str {
    id.as_str()
}

/// Option pair: (name, value).
pub type SdrOption = (Token, Token);

/// Vector of option pairs.
pub type SdrOptionVec = Vec<SdrOption>;

// ============================================================================
// SdrVersion
// ============================================================================

/// Represents a shader version with major and minor components.
///
/// Versions are used to track different iterations of shader definitions.
/// A version may be marked as "default" to indicate it's the preferred version.
///
/// # Examples
/// ```ignore
/// let v1 = SdrVersion::new(2, 1);
/// let v2 = SdrVersion::from_string("3.0");
/// assert!(v2 > v1);
/// ```
#[derive(Clone, Copy)]
pub struct SdrVersion {
    major: i32,
    minor: i32,
    is_default: bool,
}

impl SdrVersion {
    /// Creates an invalid version (0.0).
    pub fn invalid() -> Self {
        Self {
            major: 0,
            minor: 0,
            is_default: false,
        }
    }

    /// Creates a version with the given major and minor numbers.
    ///
    /// Numbers must be non-negative, and at least one must be non-zero.
    /// Returns invalid version on failure.
    pub fn new(major: i32, minor: i32) -> Self {
        if major < 0 || minor < 0 {
            // Negative numbers are invalid
            return Self::invalid();
        }
        if major == 0 && minor == 0 {
            // Both zero is invalid
            return Self::invalid();
        }
        Self {
            major,
            minor,
            is_default: false,
        }
    }

    /// Creates a version from a string like "2.1" or "3".
    ///
    /// Returns invalid version on parse failure.
    pub fn from_string(s: &str) -> Self {
        let s = s.trim();
        if s.is_empty() {
            return Self::invalid();
        }

        let parts: Vec<&str> = s.split('.').collect();
        match parts.len() {
            1 => {
                // Just major version
                if let Ok(major) = parts[0].parse::<i32>() {
                    Self::new(major, 0)
                } else {
                    Self::invalid()
                }
            }
            2 => {
                // Major.minor
                let major = parts[0].parse::<i32>().unwrap_or(-1);
                let minor = parts[1].parse::<i32>().unwrap_or(-1);
                if major >= 0 && minor >= 0 && (major > 0 || minor > 0) {
                    Self {
                        major,
                        minor,
                        is_default: false,
                    }
                } else {
                    Self::invalid()
                }
            }
            _ => Self::invalid(),
        }
    }

    /// Returns an equal version marked as default.
    ///
    /// It's permitted to mark an invalid version as the default.
    pub fn as_default(&self) -> Self {
        Self {
            major: self.major,
            minor: self.minor,
            is_default: true,
        }
    }

    /// Returns the major version number or zero for an invalid version.
    pub fn major(&self) -> i32 {
        self.major
    }

    /// Returns the minor version number or zero for an invalid version.
    pub fn minor(&self) -> i32 {
        self.minor
    }

    /// Returns true if this version is marked as default.
    pub fn is_default(&self) -> bool {
        self.is_default
    }

    /// Returns the version as a string.
    ///
    /// Matches C++: invalid → "<invalid version>", minor==0 → "N", otherwise "N.M".
    pub fn get_string(&self) -> String {
        if !self.is_valid() {
            return "<invalid version>".to_string();
        }
        if self.minor != 0 {
            format!("{}.{}", self.major, self.minor)
        } else {
            self.major.to_string()
        }
    }

    /// Returns the version as an identifier suffix.
    ///
    /// Matches C++: default or invalid → "", minor==0 → "_N", otherwise "_N.M".
    /// Note: C++ uses a dot (not underscore) between major and minor in the suffix.
    pub fn get_string_suffix(&self) -> String {
        // Default or invalid versions produce no suffix
        if self.is_default || !self.is_valid() {
            return String::new();
        }
        if self.minor != 0 {
            format!("_{}.{}", self.major, self.minor)
        } else {
            format!("_{}", self.major)
        }
    }

    /// Returns true if the version is valid (not 0.0).
    pub fn is_valid(&self) -> bool {
        self.major != 0 || self.minor != 0
    }

    /// Computes a hash for the version.
    pub fn get_hash(&self) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        let mut hasher = DefaultHasher::new();
        self.major.hash(&mut hasher);
        self.minor.hash(&mut hasher);
        hasher.finish()
    }
}

impl Default for SdrVersion {
    fn default() -> Self {
        Self::invalid()
    }
}

impl fmt::Debug for SdrVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "SdrVersion({})", self.get_string())
    }
}

impl fmt::Display for SdrVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.get_string())
    }
}

impl PartialEq for SdrVersion {
    fn eq(&self, other: &Self) -> bool {
        self.major == other.major && self.minor == other.minor
    }
}

impl Eq for SdrVersion {}

impl PartialOrd for SdrVersion {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for SdrVersion {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        match self.major.cmp(&other.major) {
            std::cmp::Ordering::Equal => self.minor.cmp(&other.minor),
            other => other,
        }
    }
}

impl Hash for SdrVersion {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.major.hash(state);
        self.minor.hash(state);
    }
}

// ============================================================================
// SdrVersionFilter
// ============================================================================

/// Enumeration used to select nodes by version.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum SdrVersionFilter {
    /// Only include default versions of nodes.
    #[default]
    DefaultOnly,
    /// Include all versions of nodes.
    AllVersions,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_creation() {
        let v = SdrVersion::new(2, 1);
        assert!(v.is_valid());
        assert_eq!(v.major(), 2);
        assert_eq!(v.minor(), 1);
        assert!(!v.is_default());
    }

    #[test]
    fn test_version_invalid() {
        let v = SdrVersion::invalid();
        assert!(!v.is_valid());
        assert_eq!(v.major(), 0);
        assert_eq!(v.minor(), 0);
    }

    #[test]
    fn test_version_from_string() {
        let v1 = SdrVersion::from_string("2.1");
        assert_eq!(v1.major(), 2);
        assert_eq!(v1.minor(), 1);

        let v2 = SdrVersion::from_string("3");
        assert_eq!(v2.major(), 3);
        assert_eq!(v2.minor(), 0);

        let v3 = SdrVersion::from_string("invalid");
        assert!(!v3.is_valid());
    }

    #[test]
    fn test_version_comparison() {
        let v1 = SdrVersion::new(1, 0);
        let v2 = SdrVersion::new(2, 0);
        let v3 = SdrVersion::new(2, 1);

        assert!(v1 < v2);
        assert!(v2 < v3);
        assert!(v1 < v3);
        assert_eq!(SdrVersion::new(2, 0), SdrVersion::new(2, 0));
    }

    #[test]
    fn test_version_as_default() {
        let v = SdrVersion::new(1, 0);
        assert!(!v.is_default());

        let vd = v.as_default();
        assert!(vd.is_default());
        assert_eq!(v, vd); // Equal for comparison purposes
    }

    #[test]
    fn test_version_string() {
        let v = SdrVersion::new(2, 1);
        assert_eq!(v.get_string(), "2.1");
        // C++ suffix format: "_N.M" (dot, not underscore)
        assert_eq!(v.get_string_suffix(), "_2.1");

        // major-only version: "_N" with no minor
        let v2 = SdrVersion::new(3, 0);
        assert_eq!(v2.get_string(), "3");
        assert_eq!(v2.get_string_suffix(), "_3");

        // invalid version
        let vi = SdrVersion::invalid();
        assert_eq!(vi.get_string(), "<invalid version>");
        assert_eq!(vi.get_string_suffix(), "");

        // default version has empty suffix
        let vd = SdrVersion::new(2, 1).as_default();
        assert_eq!(vd.get_string_suffix(), "");

        // version 0.0 edge case (invalid by definition)
        let v00 = SdrVersion::new(0, 0);
        assert!(!v00.is_valid());
        assert_eq!(v00.get_string(), "<invalid version>");
        assert_eq!(v00.get_string_suffix(), "");
    }
}
