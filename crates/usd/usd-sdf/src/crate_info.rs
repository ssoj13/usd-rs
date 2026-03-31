//! Crate file introspection for diagnostics.
//!
//! [`CrateInfo`] provides diagnostic information about .usdc (crate) binary files.
//!
//! # Examples
//!
//! ```
//! use usd_sdf::{CrateSection, CrateSummaryStats};
//!
//! let section = CrateSection::new("TOKENS", 1024, 4096);
//! assert_eq!(section.name(), "TOKENS");
//!
//! let stats = CrateSummaryStats::default();
//! assert_eq!(stats.num_specs(), 0);
//! ```

use std::fmt;

/// A section within a crate file.
///
/// Describes a named region in the binary file with its location and size.
///
/// # Examples
///
/// ```ignore
/// use usd_sdf::CrateSection;
///
/// let section = CrateSection::new("TOKENS", 1024, 4096);
/// assert_eq!(section.name(), "TOKENS");
/// assert_eq!(section.start(), 1024);
/// assert_eq!(section.size(), 4096);
/// ```
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CrateSection {
    /// Section name (e.g., "TOKENS", "PATHS", "SPECS").
    name: String,
    /// Byte offset where section starts (-1 if invalid).
    start: i64,
    /// Size in bytes (-1 if invalid).
    size: i64,
}

impl CrateSection {
    /// Creates a new section descriptor.
    #[must_use]
    pub fn new(name: impl Into<String>, start: i64, size: i64) -> Self {
        Self {
            name: name.into(),
            start,
            size,
        }
    }

    /// Creates an invalid/empty section.
    #[must_use]
    pub fn invalid() -> Self {
        Self {
            name: String::new(),
            start: -1,
            size: -1,
        }
    }

    /// Returns the section name.
    #[inline]
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the start offset in bytes.
    #[inline]
    #[must_use]
    pub fn start(&self) -> i64 {
        self.start
    }

    /// Returns the section size in bytes.
    #[inline]
    #[must_use]
    pub fn size(&self) -> i64 {
        self.size
    }

    /// Returns true if this section has valid location info.
    #[inline]
    #[must_use]
    pub fn is_valid(&self) -> bool {
        self.start >= 0 && self.size >= 0
    }
}

impl fmt::Display for CrateSection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}[{}:{}]", self.name, self.start, self.size)
    }
}

/// Summary statistics for a crate file.
///
/// Contains counts of various elements in the file for diagnostic purposes.
///
/// # Examples
///
/// ```ignore
/// use usd_sdf::CrateSummaryStats;
///
/// let mut stats = CrateSummaryStats::default();
/// stats.set_num_specs(100);
/// assert_eq!(stats.num_specs(), 100);
/// ```
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CrateSummaryStats {
    /// Number of specs (prims, properties) in the file.
    num_specs: usize,
    /// Number of unique paths.
    num_unique_paths: usize,
    /// Number of unique tokens.
    num_unique_tokens: usize,
    /// Number of unique strings.
    num_unique_strings: usize,
    /// Number of unique fields.
    num_unique_fields: usize,
    /// Number of unique field sets.
    num_unique_field_sets: usize,
}

impl CrateSummaryStats {
    /// Creates a new stats instance with all zeros.
    #[inline]
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the number of specs.
    #[inline]
    #[must_use]
    pub fn num_specs(&self) -> usize {
        self.num_specs
    }

    /// Sets the number of specs.
    #[inline]
    pub fn set_num_specs(&mut self, value: usize) {
        self.num_specs = value;
    }

    /// Returns the number of unique paths.
    #[inline]
    #[must_use]
    pub fn num_unique_paths(&self) -> usize {
        self.num_unique_paths
    }

    /// Sets the number of unique paths.
    #[inline]
    pub fn set_num_unique_paths(&mut self, value: usize) {
        self.num_unique_paths = value;
    }

    /// Returns the number of unique tokens.
    #[inline]
    #[must_use]
    pub fn num_unique_tokens(&self) -> usize {
        self.num_unique_tokens
    }

    /// Sets the number of unique tokens.
    #[inline]
    pub fn set_num_unique_tokens(&mut self, value: usize) {
        self.num_unique_tokens = value;
    }

    /// Returns the number of unique strings.
    #[inline]
    #[must_use]
    pub fn num_unique_strings(&self) -> usize {
        self.num_unique_strings
    }

    /// Sets the number of unique strings.
    #[inline]
    pub fn set_num_unique_strings(&mut self, value: usize) {
        self.num_unique_strings = value;
    }

    /// Returns the number of unique fields.
    #[inline]
    #[must_use]
    pub fn num_unique_fields(&self) -> usize {
        self.num_unique_fields
    }

    /// Sets the number of unique fields.
    #[inline]
    pub fn set_num_unique_fields(&mut self, value: usize) {
        self.num_unique_fields = value;
    }

    /// Returns the number of unique field sets.
    #[inline]
    #[must_use]
    pub fn num_unique_field_sets(&self) -> usize {
        self.num_unique_field_sets
    }

    /// Sets the number of unique field sets.
    #[inline]
    pub fn set_num_unique_field_sets(&mut self, value: usize) {
        self.num_unique_field_sets = value;
    }
}

impl fmt::Display for CrateSummaryStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "specs={}, paths={}, tokens={}, strings={}, fields={}, fieldSets={}",
            self.num_specs,
            self.num_unique_paths,
            self.num_unique_tokens,
            self.num_unique_strings,
            self.num_unique_fields,
            self.num_unique_field_sets
        )
    }
}

/// Crate file information for diagnostics.
///
/// Provides introspection into .usdc binary file structure.
/// Currently a placeholder that will be expanded when file parsing is implemented.
///
/// # Known Section Names
///
/// - `TOKENS` - Interned token strings
/// - `STRINGS` - Non-token strings  
/// - `FIELDS` - Field names
/// - `FIELDSETS` - Sets of fields per spec
/// - `PATHS` - Scene hierarchy paths
/// - `SPECS` - Spec data (prims, properties)
#[derive(Clone, Debug, Default)]
pub struct CrateInfo {
    /// File version string.
    file_version: String,
    /// Software version string.
    software_version: String,
    /// Sections in the file.
    sections: Vec<CrateSection>,
    /// Summary statistics.
    stats: CrateSummaryStats,
    /// Whether this info is valid.
    valid: bool,
}

impl CrateInfo {
    /// Creates an empty/invalid CrateInfo.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Attempts to open and read a crate file.
    ///
    /// Matches C++ `SdfCrateInfo::Open(std::string const &fileName)`.
    ///
    /// Returns an invalid `CrateInfo` if the file cannot be opened or parsed.
    pub fn open(file_name: &str) -> Self {
        use super::usdc_reader::CrateFile;
        use std::fs;

        // Read file
        let data = match fs::read(file_name) {
            Ok(d) => d,
            Err(_) => return Self::new(), // Invalid if file cannot be read
        };

        // Open crate file
        let crate_file = match CrateFile::open(&data, file_name) {
            Ok(cf) => cf,
            Err(_) => return Self::new(), // Invalid if parsing fails
        };

        // Build CrateInfo from CrateFile
        let mut info = Self::new();
        info.set_valid(true);

        // Set file version from bootstrap
        let version_str = format!(
            "{}.{}.{}",
            crate_file.version.0, crate_file.version.1, crate_file.version.2
        );
        info.set_file_version(version_str);

        // Set software version (static, from CrateFile)
        let software_version_str = format!(
            "{}.{}.{}",
            super::usdc_reader::SOFTWARE_VERSION.0,
            super::usdc_reader::SOFTWARE_VERSION.1,
            super::usdc_reader::SOFTWARE_VERSION.2
        );
        info.set_software_version(software_version_str);

        // Get sections from table of contents
        // TableOfContents stores sections internally, we need to iterate them
        // For now, we'll extract sections from known section names
        let known_sections = ["TOKENS", "STRINGS", "FIELDS", "FIELDSETS", "PATHS", "SPECS"];
        for section_name in &known_sections {
            if let Some(section) = crate_file.toc.get_section(section_name) {
                info.add_section(CrateSection::new(
                    section_name.to_string(),
                    section.start,
                    section.size,
                ));
            }
        }

        // Set summary stats
        let mut stats = CrateSummaryStats::new();
        stats.set_num_specs(crate_file.specs.len());
        stats.set_num_unique_paths(crate_file.paths.len());
        stats.set_num_unique_tokens(crate_file.tokens.len());
        stats.set_num_unique_strings(crate_file.strings.len());
        stats.set_num_unique_fields(crate_file.fields.len());
        // Count unique field sets (field_sets is a flat array with invalid-terminated groups)
        // FieldIndex wraps Index which wraps u32, invalid is u32::MAX
        // We count the number of invalid markers which separate field set groups
        use super::usdc_reader::FieldIndex;
        let invalid_marker = FieldIndex::new(u32::MAX);
        let num_field_sets = crate_file
            .field_sets
            .iter()
            .filter(|&&idx| idx == invalid_marker)
            .count();
        stats.set_num_unique_field_sets(num_field_sets);
        info.set_stats(stats);

        info
    }

    /// Returns the file format version.
    #[inline]
    #[must_use]
    pub fn file_version(&self) -> &str {
        &self.file_version
    }

    /// Sets the file version.
    #[inline]
    pub fn set_file_version(&mut self, version: impl Into<String>) {
        self.file_version = version.into();
    }

    /// Returns the software version that wrote the file.
    #[inline]
    #[must_use]
    pub fn software_version(&self) -> &str {
        &self.software_version
    }

    /// Sets the software version.
    #[inline]
    pub fn set_software_version(&mut self, version: impl Into<String>) {
        self.software_version = version.into();
    }

    /// Returns the file sections.
    #[inline]
    #[must_use]
    pub fn sections(&self) -> &[CrateSection] {
        &self.sections
    }

    /// Adds a section to this info.
    #[inline]
    pub fn add_section(&mut self, section: CrateSection) {
        self.sections.push(section);
    }

    /// Returns a mutable reference to summary statistics.
    #[inline]
    #[must_use]
    pub fn stats(&self) -> &CrateSummaryStats {
        &self.stats
    }

    /// Returns a mutable reference to summary statistics.
    #[inline]
    pub fn stats_mut(&mut self) -> &mut CrateSummaryStats {
        &mut self.stats
    }

    /// Sets the stats.
    #[inline]
    pub fn set_stats(&mut self, stats: CrateSummaryStats) {
        self.stats = stats;
    }

    /// Returns true if this info refers to a valid file.
    #[inline]
    #[must_use]
    pub fn is_valid(&self) -> bool {
        self.valid
    }

    /// Sets the validity flag.
    #[inline]
    pub fn set_valid(&mut self, valid: bool) {
        self.valid = valid;
    }

    /// Returns summary statistics structure for this file.
    ///
    /// Matches C++ `GetSummaryStats()`.
    ///
    /// Returns default stats if this info is invalid.
    #[must_use]
    pub fn get_summary_stats(&self) -> CrateSummaryStats {
        if !self.valid {
            // C++ returns default stats and logs TF_CODING_ERROR
            // For Rust, we just return default stats
            return CrateSummaryStats::default();
        }
        self.stats
    }

    /// Returns the named file sections, their location and sizes in the file.
    ///
    /// Matches C++ `GetSections()`.
    ///
    /// Returns empty vector if this info is invalid.
    #[must_use]
    pub fn get_sections(&self) -> Vec<CrateSection> {
        if !self.valid {
            // C++ returns empty vector and logs TF_CODING_ERROR
            return Vec::new();
        }
        self.sections.clone()
    }

    /// Returns the file version.
    ///
    /// Matches C++ `GetFileVersion()`.
    ///
    /// Returns empty token if this info is invalid.
    #[must_use]
    pub fn get_file_version(&self) -> usd_tf::Token {
        if !self.valid {
            // C++ returns empty TfToken and logs TF_CODING_ERROR
            return usd_tf::Token::new("");
        }
        usd_tf::Token::new(&self.file_version)
    }

    /// Returns the software version.
    ///
    /// Matches C++ `GetSoftwareVersion()`.
    ///
    /// This is a static value, not dependent on file validity.
    #[must_use]
    pub fn get_software_version() -> usd_tf::Token {
        // C++ returns CrateFile::GetSoftwareVersionToken() which is static
        // For Rust, we return the SOFTWARE_VERSION constant
        let version_str = format!(
            "{}.{}.{}",
            super::usdc_reader::SOFTWARE_VERSION.0,
            super::usdc_reader::SOFTWARE_VERSION.1,
            super::usdc_reader::SOFTWARE_VERSION.2
        );
        usd_tf::Token::new(&version_str)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_section_new() {
        let section = CrateSection::new("TOKENS", 1024, 4096);
        assert_eq!(section.name(), "TOKENS");
        assert_eq!(section.start(), 1024);
        assert_eq!(section.size(), 4096);
        assert!(section.is_valid());
    }

    #[test]
    fn test_section_invalid() {
        let section = CrateSection::invalid();
        assert!(!section.is_valid());
        assert_eq!(section.start(), -1);
        assert_eq!(section.size(), -1);
    }

    #[test]
    fn test_section_default() {
        let section = CrateSection::default();
        assert!(section.name().is_empty());
        assert_eq!(section.start(), 0);
        assert_eq!(section.size(), 0);
    }

    #[test]
    fn test_section_display() {
        let section = CrateSection::new("PATHS", 100, 200);
        let s = format!("{}", section);
        assert!(s.contains("PATHS"));
        assert!(s.contains("100"));
        assert!(s.contains("200"));
    }

    #[test]
    fn test_stats_default() {
        let stats = CrateSummaryStats::default();
        assert_eq!(stats.num_specs(), 0);
        assert_eq!(stats.num_unique_paths(), 0);
        assert_eq!(stats.num_unique_tokens(), 0);
        assert_eq!(stats.num_unique_strings(), 0);
        assert_eq!(stats.num_unique_fields(), 0);
        assert_eq!(stats.num_unique_field_sets(), 0);
    }

    #[test]
    fn test_stats_setters() {
        let mut stats = CrateSummaryStats::new();
        stats.set_num_specs(100);
        stats.set_num_unique_paths(50);
        stats.set_num_unique_tokens(200);
        stats.set_num_unique_strings(30);
        stats.set_num_unique_fields(25);
        stats.set_num_unique_field_sets(10);

        assert_eq!(stats.num_specs(), 100);
        assert_eq!(stats.num_unique_paths(), 50);
        assert_eq!(stats.num_unique_tokens(), 200);
        assert_eq!(stats.num_unique_strings(), 30);
        assert_eq!(stats.num_unique_fields(), 25);
        assert_eq!(stats.num_unique_field_sets(), 10);
    }

    #[test]
    fn test_stats_display() {
        let mut stats = CrateSummaryStats::new();
        stats.set_num_specs(10);
        let s = format!("{}", stats);
        assert!(s.contains("specs=10"));
    }

    #[test]
    fn test_crate_info_default() {
        let info = CrateInfo::new();
        assert!(!info.is_valid());
        assert!(info.file_version().is_empty());
        assert!(info.software_version().is_empty());
        assert!(info.sections().is_empty());
    }

    #[test]
    fn test_crate_info_setters() {
        let mut info = CrateInfo::new();

        info.set_file_version("0.9.0");
        info.set_software_version("usd-rs 0.1.0");
        info.add_section(CrateSection::new("TOKENS", 0, 100));
        info.set_valid(true);

        assert_eq!(info.file_version(), "0.9.0");
        assert_eq!(info.software_version(), "usd-rs 0.1.0");
        assert_eq!(info.sections().len(), 1);
        assert!(info.is_valid());
    }

    #[test]
    fn test_crate_info_stats() {
        let mut info = CrateInfo::new();
        info.stats_mut().set_num_specs(42);
        assert_eq!(info.stats().num_specs(), 42);
    }
}
