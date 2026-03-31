//! File format version parsing and comparison.
//!
//! [`FileVersion`] holds, parses, and compares file format versions.
//! Used by both crate (binary) and text file formats.
//!
//! # Version Scheme
//!
//! USD uses semantic versioning with major.minor.patch:
//! - Major version changes break compatibility
//! - Minor version changes are forward-compatible within the same major
//! - Patch version changes are always forward-compatible
//!
//! # Examples
//!
//! ```
//! use usd_sdf::FileVersion;
//!
//! let v = FileVersion::new(1, 4, 32);
//! assert_eq!(v.major(), 1);
//! assert_eq!(v.minor(), 4);
//! assert_eq!(v.patch(), 32);
//!
//! let v2: FileVersion = "1.4.32".parse().unwrap();
//! assert_eq!(v, v2);
//! ```

use std::fmt;
use std::str::FromStr;

/// File format version with major, minor, and patch components.
///
/// This type is used for file format versioning in USD layer files.
/// Versions follow semantic versioning rules for compatibility.
///
/// # Examples
///
/// ```ignore
/// use usd_sdf::FileVersion;
///
/// let current = FileVersion::new(1, 4, 32);
/// let file = FileVersion::new(1, 4, 0);
///
/// // Can read files with same major and <= minor version
/// assert!(current.can_read(&file));
///
/// // Can write to files with same major and <= minor.patch
/// assert!(current.can_write(&file));
/// ```
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct FileVersion {
    /// Major version number.
    major: u8,
    /// Minor version number.
    minor: u8,
    /// Patch version number.
    patch: u8,
}

impl FileVersion {
    /// Creates a new file version.
    #[inline]
    #[must_use]
    pub const fn new(major: u8, minor: u8, patch: u8) -> Self {
        Self {
            major,
            minor,
            patch,
        }
    }

    /// Creates a version from a byte array (for crate file headers).
    ///
    /// The array must have at least 3 elements: [major, minor, patch].
    #[inline]
    #[must_use]
    pub const fn from_bytes(version: [u8; 3]) -> Self {
        Self {
            major: version[0],
            minor: version[1],
            patch: version[2],
        }
    }

    /// Parses a version from a dot-separated string like "1.4.32".
    ///
    /// Returns `None` if the string is malformed.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use usd_sdf::FileVersion;
    ///
    /// assert_eq!(FileVersion::parse("1.4.32"), Some(FileVersion::new(1, 4, 32)));
    /// assert_eq!(FileVersion::parse("1.0"), Some(FileVersion::new(1, 0, 0)));
    /// assert_eq!(FileVersion::parse("invalid"), None);
    /// ```
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        let parts: Vec<&str> = s.trim().split('.').collect();

        if parts.is_empty() || parts.len() > 3 {
            return None;
        }

        let major = parts[0].parse::<u8>().ok()?;
        let minor = parts.get(1).and_then(|s| s.parse::<u8>().ok()).unwrap_or(0);
        let patch = parts.get(2).and_then(|s| s.parse::<u8>().ok()).unwrap_or(0);

        Some(Self {
            major,
            minor,
            patch,
        })
    }

    /// Returns the major version number.
    #[inline]
    #[must_use]
    pub const fn major(&self) -> u8 {
        self.major
    }

    /// Returns the minor version number.
    #[inline]
    #[must_use]
    pub const fn minor(&self) -> u8 {
        self.minor
    }

    /// Returns the patch version number.
    #[inline]
    #[must_use]
    pub const fn patch(&self) -> u8 {
        self.patch
    }

    /// Returns the version as a single 32-bit integer for comparison.
    ///
    /// Format: 0x00MMNNPP where MM=major, NN=minor, PP=patch.
    #[inline]
    #[must_use]
    pub const fn as_int(&self) -> u32 {
        ((self.major as u32) << 16) | ((self.minor as u32) << 8) | (self.patch as u32)
    }

    /// Returns true if this version is valid (not all zeros).
    #[inline]
    #[must_use]
    pub const fn is_valid(&self) -> bool {
        self.as_int() != 0
    }

    /// Returns the version as a string, excluding patch if zero.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use usd_sdf::FileVersion;
    ///
    /// assert_eq!(FileVersion::new(1, 4, 0).as_string(), "1.4");
    /// assert_eq!(FileVersion::new(1, 4, 32).as_string(), "1.4.32");
    /// ```
    #[must_use]
    pub fn as_string(&self) -> String {
        if self.patch == 0 {
            format!("{}.{}", self.major, self.minor)
        } else {
            format!("{}.{}.{}", self.major, self.minor, self.patch)
        }
    }

    /// Returns the full version string, always including patch.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use usd_sdf::FileVersion;
    ///
    /// assert_eq!(FileVersion::new(1, 4, 0).as_full_string(), "1.4.0");
    /// ```
    #[must_use]
    pub fn as_full_string(&self) -> String {
        format!("{}.{}.{}", self.major, self.minor, self.patch)
    }

    /// Returns true if we can read a file with the given version.
    ///
    /// Reading is allowed if:
    /// - Same major version
    /// - File's minor version <= our minor version
    ///
    /// Patch version is irrelevant for read compatibility.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use usd_sdf::FileVersion;
    ///
    /// let current = FileVersion::new(1, 4, 0);
    /// assert!(current.can_read(&FileVersion::new(1, 3, 0))); // older minor OK
    /// assert!(current.can_read(&FileVersion::new(1, 4, 99))); // same minor OK
    /// assert!(!current.can_read(&FileVersion::new(1, 5, 0))); // newer minor not OK
    /// assert!(!current.can_read(&FileVersion::new(2, 0, 0))); // different major not OK
    /// ```
    #[inline]
    #[must_use]
    pub const fn can_read(&self, file_ver: &FileVersion) -> bool {
        file_ver.major == self.major && file_ver.minor <= self.minor
    }

    /// Returns true if we can write a file with the given version.
    ///
    /// Writing is allowed if:
    /// - Same major version
    /// - File's minor version < our minor version, OR
    /// - Same minor version and file's patch <= our patch
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use usd_sdf::FileVersion;
    ///
    /// let current = FileVersion::new(1, 4, 32);
    /// assert!(current.can_write(&FileVersion::new(1, 3, 99))); // older minor OK
    /// assert!(current.can_write(&FileVersion::new(1, 4, 32))); // same OK
    /// assert!(current.can_write(&FileVersion::new(1, 4, 0))); // same minor, older patch OK
    /// assert!(!current.can_write(&FileVersion::new(1, 4, 33))); // newer patch not OK
    /// ```
    #[inline]
    #[must_use]
    pub const fn can_write(&self, file_ver: &FileVersion) -> bool {
        file_ver.major == self.major
            && (file_ver.minor < self.minor
                || (file_ver.minor == self.minor && file_ver.patch <= self.patch))
    }
}

impl PartialOrd for FileVersion {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for FileVersion {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.as_int().cmp(&other.as_int())
    }
}

/// Error type for parsing FileVersion from string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseFileVersionError;

impl fmt::Display for ParseFileVersionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid file version string")
    }
}

impl std::error::Error for ParseFileVersionError {}

impl FromStr for FileVersion {
    type Err = ParseFileVersionError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s).ok_or(ParseFileVersionError)
    }
}

impl fmt::Display for FileVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new() {
        let v = FileVersion::new(1, 4, 32);
        assert_eq!(v.major(), 1);
        assert_eq!(v.minor(), 4);
        assert_eq!(v.patch(), 32);
    }

    #[test]
    fn test_default() {
        let v = FileVersion::default();
        assert_eq!(v.major(), 0);
        assert_eq!(v.minor(), 0);
        assert_eq!(v.patch(), 0);
        assert!(!v.is_valid());
    }

    #[test]
    fn test_from_bytes() {
        let v = FileVersion::from_bytes([1, 4, 32]);
        assert_eq!(v, FileVersion::new(1, 4, 32));
    }

    #[test]
    fn test_parse() {
        assert_eq!(
            FileVersion::parse("1.4.32"),
            Some(FileVersion::new(1, 4, 32))
        );
        assert_eq!(FileVersion::parse("1.4"), Some(FileVersion::new(1, 4, 0)));
        assert_eq!(FileVersion::parse("1"), Some(FileVersion::new(1, 0, 0)));
        assert_eq!(FileVersion::parse(""), None);
        assert_eq!(FileVersion::parse("invalid"), None);
        assert_eq!(FileVersion::parse("1.2.3.4"), None);
    }

    #[test]
    fn test_from_str_trait() {
        assert_eq!(
            "1.4.32".parse::<FileVersion>(),
            Ok(FileVersion::new(1, 4, 32))
        );
        assert!("invalid".parse::<FileVersion>().is_err());
    }

    #[test]
    fn test_as_int() {
        let v = FileVersion::new(1, 4, 32);
        assert_eq!(v.as_int(), 0x00010420);
    }

    #[test]
    fn test_is_valid() {
        assert!(FileVersion::new(1, 0, 0).is_valid());
        assert!(FileVersion::new(0, 1, 0).is_valid());
        assert!(FileVersion::new(0, 0, 1).is_valid());
        assert!(!FileVersion::new(0, 0, 0).is_valid());
    }

    #[test]
    fn test_as_string() {
        assert_eq!(FileVersion::new(1, 4, 0).as_string(), "1.4");
        assert_eq!(FileVersion::new(1, 4, 32).as_string(), "1.4.32");
        assert_eq!(FileVersion::new(1, 0, 0).as_string(), "1.0");
    }

    #[test]
    fn test_as_full_string() {
        assert_eq!(FileVersion::new(1, 4, 0).as_full_string(), "1.4.0");
        assert_eq!(FileVersion::new(1, 4, 32).as_full_string(), "1.4.32");
    }

    #[test]
    fn test_can_read() {
        let current = FileVersion::new(1, 4, 0);

        // Same major, older or same minor - OK
        assert!(current.can_read(&FileVersion::new(1, 0, 0)));
        assert!(current.can_read(&FileVersion::new(1, 3, 99)));
        assert!(current.can_read(&FileVersion::new(1, 4, 0)));
        assert!(current.can_read(&FileVersion::new(1, 4, 99))); // patch ignored

        // Newer minor - not OK
        assert!(!current.can_read(&FileVersion::new(1, 5, 0)));

        // Different major - not OK
        assert!(!current.can_read(&FileVersion::new(0, 4, 0)));
        assert!(!current.can_read(&FileVersion::new(2, 0, 0)));
    }

    #[test]
    fn test_can_write() {
        let current = FileVersion::new(1, 4, 32);

        // Same major, older minor - OK
        assert!(current.can_write(&FileVersion::new(1, 3, 99)));

        // Same major and minor, older or same patch - OK
        assert!(current.can_write(&FileVersion::new(1, 4, 0)));
        assert!(current.can_write(&FileVersion::new(1, 4, 32)));

        // Same major and minor, newer patch - not OK
        assert!(!current.can_write(&FileVersion::new(1, 4, 33)));

        // Newer minor - not OK
        assert!(!current.can_write(&FileVersion::new(1, 5, 0)));

        // Different major - not OK
        assert!(!current.can_write(&FileVersion::new(2, 0, 0)));
    }

    #[test]
    fn test_ordering() {
        let v1 = FileVersion::new(1, 0, 0);
        let v2 = FileVersion::new(1, 1, 0);
        let v3 = FileVersion::new(1, 1, 1);
        let v4 = FileVersion::new(2, 0, 0);

        assert!(v1 < v2);
        assert!(v2 < v3);
        assert!(v3 < v4);
        assert!(v1 <= v1);
        assert!(v1 >= v1);
        assert!(v4 > v1);
    }

    #[test]
    fn test_display() {
        let v = FileVersion::new(1, 4, 32);
        assert_eq!(format!("{}", v), "1.4.32");

        let v2 = FileVersion::new(1, 4, 0);
        assert_eq!(format!("{}", v2), "1.4");
    }
}
