//! File format abstraction for USD layers.
//!
//! This module defines the base interface for reading and writing USD files
//! in various formats (usda, usdc, usdz, etc.). The [`FileFormat`] trait
//! provides the core abstraction that all format implementations must follow.
//!
//! # Architecture
//!
//! The file format system consists of:
//!
//! 1. **FileFormat trait** - The base interface all formats must implement
//! 2. **FileFormatArguments** - Format-specific parameters (e.g., target schema)
//! 3. **FileFormatError** - Errors that can occur during format operations
//! 4. **Format registry** - Global registry of available formats
//!
//! # File Format Arguments
//!
//! Formats can accept arguments to customize behavior. Common arguments include:
//!
//! - `"target"` - Schema target (e.g., "usd", "usdGeom")
//! - `"format"` - Force a specific format when multiple handle same extension
//!
//! # Format Discovery
//!
//! Formats are discovered by file extension and can be looked up via:
//!
//! - [`find_format_by_extension`] - Find format by extension and optional target
//! - [`find_format_by_id`] - Find format by unique identifier
//! - [`get_all_formats`] - Get all registered formats
//!
//! # Examples
//!
//! ## Implementing a Custom Format
//!
//! ```ignore
//! use usd_sdf::{FileFormat, FileFormatArguments, FileFormatError, Layer};
//! use usd_tf::Token;
//! use usd_ar::ResolvedPath;
//!
//! struct MyFormat;
//!
//! impl FileFormat for MyFormat {
//!     fn format_id(&self) -> Token {
//!         Token::new("myformat")
//!     }
//!
//!     fn target(&self) -> Token {
//!         Token::new("usd")
//!     }
//!
//!     fn file_extensions(&self) -> Vec<String> {
//!         vec!["myext".to_string()]
//!     }
//!
//!     fn can_read(&self, path: &str) -> bool {
//!         path.ends_with(".myext")
//!     }
//!
//!     fn read(
//!         &self,
//!         layer: &mut Layer,
//!         resolved_path: &ResolvedPath,
//!         metadata_only: bool,
//!     ) -> Result<(), FileFormatError> {
//!         // Implementation...
//!         Ok(())
//!     }
//!
//!     // ... other methods
//! }
//! ```
//!
//! ## Reading a Layer
//!
//! ```ignore
//! use usd_sdf::{find_format_by_extension, Layer};
//! use usd_ar::ResolvedPath;
//!
//! let path = ResolvedPath::new("/path/to/file.usda");
//! if let Some(format) = find_format_by_extension("usda", None) {
//!     let mut layer = Layer::new();
//!     format.read(&mut layer, &path, false)?;
//! }
//! ```

use std::collections::HashMap;
use std::error::Error;
use std::fmt;
use std::sync::{Arc, OnceLock, RwLock};

use usd_ar::ResolvedPath;
use usd_plug::PlugRegistry;
use usd_tf::Token;

use super::Layer;

// ============================================================================
// FileFormatArguments - Format-specific arguments
// ============================================================================

/// Arguments passed to file format operations.
///
/// File format arguments allow customizing format behavior without
/// requiring format-specific APIs. Common arguments include:
///
/// - `"target"` - Target schema (e.g., "usd", "usdGeom")
/// - `"format"` - Force specific format when extension is ambiguous
///
/// # Examples
///
/// ```ignore
/// use usd_sdf::FileFormatArguments;
///
/// let mut args = FileFormatArguments::new();
/// args.insert("target", "usdGeom");
/// args.insert("compression", "lz4");
///
/// assert_eq!(args.get("target"), Some(&"usdGeom".to_string()));
/// ```
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FileFormatArguments {
    /// Internal storage for key-value arguments
    args: HashMap<String, String>,
}

impl FileFormatArguments {
    /// Creates a new empty set of file format arguments.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use usd_sdf::FileFormatArguments;
    ///
    /// let args = FileFormatArguments::new();
    /// assert!(args.is_empty());
    /// ```
    #[must_use]
    pub fn new() -> Self {
        Self {
            args: HashMap::new(),
        }
    }

    /// Inserts a key-value argument.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use usd_sdf::FileFormatArguments;
    ///
    /// let mut args = FileFormatArguments::new();
    /// args.insert("target", "usd");
    /// assert_eq!(args.get("target"), Some(&"usd".to_string()));
    /// ```
    pub fn insert(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.args.insert(key.into(), value.into());
    }

    /// Gets the value for a key.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use usd_sdf::FileFormatArguments;
    ///
    /// let mut args = FileFormatArguments::new();
    /// args.insert("target", "usd");
    /// assert_eq!(args.get("target"), Some(&"usd".to_string()));
    /// assert_eq!(args.get("missing"), None);
    /// ```
    #[must_use]
    pub fn get(&self, key: &str) -> Option<&String> {
        self.args.get(key)
    }

    /// Checks if arguments contain a key.
    #[must_use]
    pub fn contains_key(&self, key: &str) -> bool {
        self.args.contains_key(key)
    }

    /// Removes a key-value pair.
    pub fn remove(&mut self, key: &str) -> Option<String> {
        self.args.remove(key)
    }

    /// Returns true if there are no arguments.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.args.is_empty()
    }

    /// Returns the number of arguments.
    #[must_use]
    pub fn len(&self) -> usize {
        self.args.len()
    }

    /// Clears all arguments.
    pub fn clear(&mut self) {
        self.args.clear();
    }

    /// Returns an iterator over the arguments.
    pub fn iter(&self) -> impl Iterator<Item = (&String, &String)> {
        self.args.iter()
    }

    /// Gets the target argument if present.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use usd_sdf::FileFormatArguments;
    ///
    /// let mut args = FileFormatArguments::new();
    /// args.insert("target", "usdGeom");
    /// assert_eq!(args.target(), Some("usdGeom"));
    /// ```
    #[must_use]
    pub fn target(&self) -> Option<&str> {
        self.get("target").map(String::as_str)
    }

    /// Sets the target argument.
    pub fn set_target(&mut self, target: impl Into<String>) {
        self.insert("target", target);
    }
}

impl From<HashMap<String, String>> for FileFormatArguments {
    fn from(args: HashMap<String, String>) -> Self {
        Self { args }
    }
}

impl From<FileFormatArguments> for HashMap<String, String> {
    fn from(args: FileFormatArguments) -> Self {
        args.args
    }
}

// ============================================================================
// FileFormatError - Error type for file format operations
// ============================================================================

/// Errors that can occur during file format operations.
///
/// # Examples
///
/// ```ignore
/// use usd_sdf::FileFormatError;
///
/// let err = FileFormatError::unsupported_format("unknown", "xyz");
/// assert!(err.to_string().contains("xyz"));
/// ```
#[derive(Debug, Clone)]
pub enum FileFormatError {
    /// File could not be read.
    ReadError {
        /// Path that failed to read
        path: String,
        /// Reason for failure
        reason: String,
    },

    /// File could not be written.
    WriteError {
        /// Path that failed to write
        path: String,
        /// Reason for failure
        reason: String,
    },

    /// Unsupported file format.
    UnsupportedFormat {
        /// File extension
        extension: String,
        /// Optional additional context
        context: String,
    },

    /// Format version mismatch.
    VersionMismatch {
        /// Expected version
        expected: String,
        /// Found version
        found: String,
    },

    /// Corrupt or malformed file.
    CorruptFile {
        /// Path to corrupt file
        path: String,
        /// Details about corruption
        details: String,
    },

    /// Missing required data.
    MissingData {
        /// What data is missing
        description: String,
    },

    /// Invalid arguments provided.
    InvalidArguments {
        /// Description of invalid arguments
        description: String,
    },

    /// I/O error occurred.
    IoError {
        /// Path involved in I/O operation
        path: String,
        /// Error message
        error: String,
    },

    /// Generic error.
    Other(String),
}

impl FileFormatError {
    /// Creates a read error.
    #[must_use]
    pub fn read_error(path: impl Into<String>, reason: impl Into<String>) -> Self {
        Self::ReadError {
            path: path.into(),
            reason: reason.into(),
        }
    }

    /// Creates a write error.
    #[must_use]
    pub fn write_error(path: impl Into<String>, reason: impl Into<String>) -> Self {
        Self::WriteError {
            path: path.into(),
            reason: reason.into(),
        }
    }

    /// Creates an unsupported format error.
    #[must_use]
    pub fn unsupported_format(extension: impl Into<String>, context: impl Into<String>) -> Self {
        Self::UnsupportedFormat {
            extension: extension.into(),
            context: context.into(),
        }
    }

    /// Creates a version mismatch error.
    #[must_use]
    pub fn version_mismatch(expected: impl Into<String>, found: impl Into<String>) -> Self {
        Self::VersionMismatch {
            expected: expected.into(),
            found: found.into(),
        }
    }

    /// Creates a corrupt file error.
    #[must_use]
    pub fn corrupt_file(path: impl Into<String>, details: impl Into<String>) -> Self {
        Self::CorruptFile {
            path: path.into(),
            details: details.into(),
        }
    }

    /// Creates a missing data error.
    #[must_use]
    pub fn missing_data(description: impl Into<String>) -> Self {
        Self::MissingData {
            description: description.into(),
        }
    }

    /// Creates an invalid arguments error.
    #[must_use]
    pub fn invalid_arguments(description: impl Into<String>) -> Self {
        Self::InvalidArguments {
            description: description.into(),
        }
    }

    /// Creates an I/O error.
    #[must_use]
    pub fn io_error(path: impl Into<String>, error: impl Into<String>) -> Self {
        Self::IoError {
            path: path.into(),
            error: error.into(),
        }
    }

    /// Creates a generic error.
    #[must_use]
    pub fn other(msg: impl Into<String>) -> Self {
        Self::Other(msg.into())
    }
}

impl fmt::Display for FileFormatError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ReadError { path, reason } => {
                write!(f, "Failed to read '{}': {}", path, reason)
            }
            Self::WriteError { path, reason } => {
                write!(f, "Failed to write '{}': {}", path, reason)
            }
            Self::UnsupportedFormat { extension, context } => {
                if context.is_empty() {
                    write!(f, "Unsupported file format: .{}", extension)
                } else {
                    write!(f, "Unsupported file format .{}: {}", extension, context)
                }
            }
            Self::VersionMismatch { expected, found } => {
                write!(
                    f,
                    "Version mismatch: expected {}, found {}",
                    expected, found
                )
            }
            Self::CorruptFile { path, details } => {
                write!(f, "Corrupt file '{}': {}", path, details)
            }
            Self::MissingData { description } => {
                write!(f, "Missing required data: {}", description)
            }
            Self::InvalidArguments { description } => {
                write!(f, "Invalid arguments: {}", description)
            }
            Self::IoError { path, error } => {
                write!(f, "I/O error on '{}': {}", path, error)
            }
            Self::Other(msg) => write!(f, "File format error: {}", msg),
        }
    }
}

impl Error for FileFormatError {}

impl From<std::io::Error> for FileFormatError {
    fn from(err: std::io::Error) -> Self {
        Self::IoError {
            path: String::new(),
            error: err.to_string(),
        }
    }
}

// ============================================================================
// FileFormat - Base trait for all file formats
// ============================================================================

/// Base interface for USD file format implementations.
///
/// The `FileFormat` trait defines the interface that all USD file formats
/// must implement. This includes text formats (usda), binary formats (usdc),
/// and package formats (usdz).
///
/// # Required Methods
///
/// All formats must provide:
///
/// - Identification: [`format_id`](FileFormat::format_id), [`target`](FileFormat::target)
/// - Extension info: [`file_extensions`](FileFormat::file_extensions)
/// - Capabilities: [`can_read`](FileFormat::can_read)
/// - I/O: [`read`](FileFormat::read)
///
/// # Optional Methods
///
/// Most methods have default implementations:
///
/// - Writing: [`write_to_file`](FileFormat::write_to_file), [`write_to_string`](FileFormat::write_to_string)
/// - Package support: [`is_package`](FileFormat::is_package)
/// - Arguments: [`get_file_format_arguments`](FileFormat::get_file_format_arguments)
///
/// # Thread Safety
///
/// Implementations must be `Send + Sync` as formats are shared globally.
///
/// # Examples
///
/// See module-level documentation for implementation examples.
pub trait FileFormat: Send + Sync {
    /// Returns the unique identifier for this file format.
    ///
    /// The format ID distinguishes this format from others that may
    /// handle the same file extension (e.g., "usda" vs "usda_text").
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use usd_sdf::FileFormat;
    /// use usd_tf::Token;
    ///
    /// struct MyFormat;
    /// impl FileFormat for MyFormat {
    ///     fn format_id(&self) -> Token {
    ///         Token::new("myformat")
    ///     }
    ///     // ... other required methods
    /// }
    /// ```
    fn format_id(&self) -> Token;

    /// Returns the target schema for this format.
    ///
    /// The target identifies which schema this format is designed for.
    /// Common targets include "usd", "usdGeom", "usdShade", etc.
    /// An empty token indicates the format works with any schema.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// fn target(&self) -> Token {
    ///     Token::new("usd")
    /// }
    /// ```
    fn target(&self) -> Token;

    /// Returns the file extensions supported by this format.
    ///
    /// Extensions should be returned without the leading dot.
    /// The first extension is considered the primary extension.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// fn file_extensions(&self) -> Vec<String> {
    ///     vec!["usda".to_string(), "usd".to_string()]
    /// }
    /// ```
    fn file_extensions(&self) -> Vec<String>;

    /// Returns the primary file extension.
    ///
    /// This is the extension used when creating new files.
    /// Default implementation returns the first extension.
    fn primary_file_extension(&self) -> String {
        self.file_extensions().first().cloned().unwrap_or_default()
    }

    /// Checks if this format supports the given extension.
    ///
    /// # Arguments
    ///
    /// * `extension` - File extension without leading dot
    ///
    /// # Examples
    ///
    /// ```ignore
    /// assert!(format.is_supported_extension("usda"));
    /// assert!(!format.is_supported_extension("txt"));
    /// ```
    fn is_supported_extension(&self, extension: &str) -> bool {
        self.file_extensions()
            .iter()
            .any(|ext| ext.eq_ignore_ascii_case(extension))
    }

    /// Returns true if this format represents a package.
    ///
    /// Package formats (like usdz) contain multiple files bundled together.
    /// Default implementation returns false.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// fn is_package(&self) -> bool {
    ///     false  // Most formats are not packages
    /// }
    /// ```
    fn is_package(&self) -> bool {
        false
    }

    /// Returns the path to the root layer within a package.
    ///
    /// Only relevant for package formats. For non-package formats,
    /// returns an empty string.
    ///
    /// # Arguments
    ///
    /// * `resolved_path` - Path to the package file
    fn get_package_root_layer_path(&self, _resolved_path: &ResolvedPath) -> String {
        String::new()
    }

    /// Returns true if this format can read the specified file.
    ///
    /// This should perform a quick check (e.g., extension or magic number)
    /// without fully parsing the file.
    ///
    /// # Arguments
    ///
    /// * `path` - File path to check
    ///
    /// # Examples
    ///
    /// ```ignore
    /// fn can_read(&self, path: &str) -> bool {
    ///     path.ends_with(".usda") || path.ends_with(".usd")
    /// }
    /// ```
    fn can_read(&self, path: &str) -> bool;

    /// Returns true if this format can write to the specified path.
    ///
    /// Default implementation delegates to `can_read`.
    fn can_write(&self, path: &str) -> bool {
        self.can_read(path)
    }

    /// Reads scene description from a file into a layer.
    ///
    /// # Arguments
    ///
    /// * `layer` - Layer to populate with data
    /// * `resolved_path` - Physical path to the file
    /// * `metadata_only` - If true, only read layer metadata (optimization hint)
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - File cannot be opened or read
    /// - File format is invalid or corrupt
    /// - Required data is missing
    ///
    /// # Examples
    ///
    /// ```ignore
    /// fn read(
    ///     &self,
    ///     layer: &mut Layer,
    ///     resolved_path: &ResolvedPath,
    ///     metadata_only: bool,
    /// ) -> Result<(), FileFormatError> {
    ///     // Read file contents
    ///     // Parse and populate layer
    ///     Ok(())
    /// }
    /// ```
    fn read(
        &self,
        layer: &mut Layer,
        resolved_path: &ResolvedPath,
        metadata_only: bool,
    ) -> Result<(), FileFormatError>;

    /// Writes layer content to a file.
    ///
    /// # Arguments
    ///
    /// * `layer` - Layer to write
    /// * `file_path` - Destination file path
    /// * `comment` - Optional comment to include in file
    /// * `args` - Format-specific arguments
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be written.
    ///
    /// # Default Implementation
    ///
    /// Returns an error indicating writing is not supported.
    fn write_to_file(
        &self,
        _layer: &Layer,
        file_path: &str,
        _comment: Option<&str>,
        _args: &FileFormatArguments,
    ) -> Result<(), FileFormatError> {
        Err(FileFormatError::write_error(
            file_path,
            "Write not supported for this format",
        ))
    }

    /// Writes layer content to a string.
    ///
    /// # Arguments
    ///
    /// * `layer` - Layer to write
    /// * `comment` - Optional comment to include
    ///
    /// # Errors
    ///
    /// Returns an error if serialization fails.
    ///
    /// # Default Implementation
    ///
    /// Returns an error indicating string output is not supported.
    fn write_to_string(
        &self,
        _layer: &Layer,
        _comment: Option<&str>,
    ) -> Result<String, FileFormatError> {
        Err(FileFormatError::other(
            "String output not supported for this format",
        ))
    }

    /// Returns file format arguments parsed from the file path.
    ///
    /// Some formats embed arguments in the path (e.g., `file.usd:SDF_FORMAT_ARGS:target=usdGeom`).
    /// This method extracts those arguments.
    ///
    /// # Arguments
    ///
    /// * `path` - File path potentially containing arguments
    ///
    /// # Default Implementation
    ///
    /// Returns empty arguments.
    fn get_file_format_arguments(&self, _path: &str) -> FileFormatArguments {
        FileFormatArguments::new()
    }

    /// Returns the default file format arguments.
    ///
    /// These arguments are used when no explicit arguments are provided.
    ///
    /// # Default Implementation
    ///
    /// Returns empty arguments.
    fn get_default_file_format_arguments(&self) -> FileFormatArguments {
        FileFormatArguments::new()
    }

    /// Returns the file cookie string for this format.
    ///
    /// The cookie is a magic string at the beginning of files that
    /// identifies the format (e.g., "#usda 1.0").
    ///
    /// # Default Implementation
    ///
    /// Returns an empty string.
    fn get_file_cookie(&self) -> String {
        String::new()
    }

    /// Returns the version string for this format.
    ///
    /// # Default Implementation
    ///
    /// Returns "1.0".
    fn get_version_string(&self) -> Token {
        Token::new("1.0")
    }

    /// Returns true if this format supports reading.
    ///
    /// # Default Implementation
    ///
    /// Returns true (all formats that implement `read` support reading).
    fn supports_reading(&self) -> bool {
        true
    }

    /// Returns true if this format supports writing.
    ///
    /// # Default Implementation
    ///
    /// Returns false (formats must opt-in to writing support).
    fn supports_writing(&self) -> bool {
        false
    }

    /// Returns true if this format supports editing.
    ///
    /// Editing means the format can roundtrip data without loss.
    ///
    /// # Default Implementation
    ///
    /// Delegates to `supports_writing`.
    fn supports_editing(&self) -> bool {
        self.supports_writing()
    }

    /// Returns true if this file format supports dynamic file format arguments.
    ///
    /// Dynamic file formats compute their arguments from composed field values
    /// at runtime during composition. This is used for payloads that need
    /// context-dependent arguments.
    ///
    /// # Default Implementation
    ///
    /// Returns false. Formats that implement dynamic arguments should
    /// override this method.
    fn is_dynamic(&self) -> bool {
        false
    }
}

// ============================================================================
// Format Registry - Global format management
// ============================================================================

/// Metadata discovered from a plugin's plugInfo.json for a file format type.
///
/// Populated by `discover_format_plugins` from PlugRegistry. Does not represent
/// a loaded format — just the declared metadata. Used to expose plugin-declared
/// extensions/IDs alongside manually registered formats.
struct PluginFormatInfo {
    format_id: Token,
    extensions: Vec<String>,
    target: Token,
    is_primary: bool,
    plugin_name: String,
}

/// Global registry of file formats.
///
/// Matches C++ `SdfFileFormatRegistry`. Provides type-safe access to
/// the set of registered file formats indexed by extension and format ID.
pub struct FileFormatRegistry {
    /// Formats indexed by extension and target
    formats: HashMap<String, Vec<Arc<dyn FileFormat>>>,
    /// Formats indexed by format ID
    formats_by_id: HashMap<Token, Arc<dyn FileFormat>>,
    /// Metadata-only entries discovered from PlugRegistry (no loaded format impl)
    plugin_formats: Vec<PluginFormatInfo>,
}

impl FileFormatRegistry {
    /// Creates a new empty registry.
    pub fn new() -> Self {
        Self {
            formats: HashMap::new(),
            formats_by_id: HashMap::new(),
            plugin_formats: Vec::new(),
        }
    }

    /// Scans PlugRegistry for all types derived from "SdfFileFormat" and
    /// records their metadata in `plugin_formats`.
    ///
    /// This mirrors C++ `Sdf_FileFormatRegistry::_RegisterFormatPlugins`
    /// but does NOT instantiate or load format implementations — it only
    /// collects metadata so that callers can see which extensions/IDs are
    /// declared by plugins.
    pub fn discover_format_plugins(&mut self) {
        let plug_reg = PlugRegistry::get_instance();
        let derived = plug_reg.get_all_derived_types("SdfFileFormat");

        for type_name in &derived {
            // Retrieve the per-type metadata dict from the plugin that owns this type.
            let format_id_val = match plug_reg.get_data_from_plugin_metadata(type_name, "formatId")
            {
                Some(v) => v,
                None => {
                    log::debug!(
                        "discover_format_plugins: no formatId for type '{}', skipping",
                        type_name
                    );
                    continue;
                }
            };

            let format_id_str = match format_id_val.as_string() {
                Some(s) if !s.is_empty() => s.to_string(),
                _ => {
                    log::debug!(
                        "discover_format_plugins: formatId for type '{}' is not a non-empty string, skipping",
                        type_name
                    );
                    continue;
                }
            };

            let extensions_val =
                match plug_reg.get_data_from_plugin_metadata(type_name, "extensions") {
                    Some(v) => v,
                    None => {
                        log::debug!(
                            "discover_format_plugins: no extensions for type '{}', skipping",
                            type_name
                        );
                        continue;
                    }
                };

            let extensions: Vec<String> = match extensions_val.as_array() {
                Some(arr) => arr
                    .iter()
                    .filter_map(|v| v.as_string().map(|s| s.to_lowercase()))
                    .filter(|s| !s.is_empty())
                    .map(|s| s.trim_start_matches('.').to_string())
                    .collect(),
                None => {
                    log::debug!(
                        "discover_format_plugins: extensions for type '{}' is not an array, skipping",
                        type_name
                    );
                    continue;
                }
            };

            if extensions.is_empty() {
                log::debug!(
                    "discover_format_plugins: empty extensions list for type '{}', skipping",
                    type_name
                );
                continue;
            }

            // "target" walks the type hierarchy; for simplicity read it directly
            // (inherited target is uncommon in practice for built-in formats).
            let target_str = plug_reg
                .get_data_from_plugin_metadata(type_name, "target")
                .and_then(|v| v.as_string().map(|s| s.to_string()))
                .unwrap_or_default();

            // Skip types with no target (matches C++ behavior — required field).
            if target_str.is_empty() {
                log::debug!(
                    "discover_format_plugins: no target for type '{}', skipping",
                    type_name
                );
                continue;
            }

            let is_primary = plug_reg
                .get_data_from_plugin_metadata(type_name, "primary")
                .and_then(|v| v.as_bool())
                .unwrap_or(true);

            let plugin_name = plug_reg
                .get_plugin_for_type(type_name)
                .map(|p| p.get_name().to_string())
                .unwrap_or_default();

            // Skip duplicate format IDs that are already manually registered.
            let id_token = Token::new(&format_id_str);
            if self.formats_by_id.contains_key(&id_token) {
                log::debug!(
                    "discover_format_plugins: format '{}' already manually registered, skipping plugin metadata",
                    format_id_str
                );
                continue;
            }

            log::debug!(
                "discover_format_plugins: found '{}' exts={:?} target='{}' primary={} plugin='{}'",
                format_id_str,
                extensions,
                target_str,
                is_primary,
                plugin_name
            );

            self.plugin_formats.push(PluginFormatInfo {
                format_id: id_token,
                extensions,
                target: Token::new(&target_str),
                is_primary,
                plugin_name,
            });
        }
    }

    /// Registers a new file format.
    pub fn register(&mut self, format: Arc<dyn FileFormat>) {
        // Register by format ID
        let id = format.format_id();
        self.formats_by_id.insert(id.clone(), format.clone());

        // Register by each extension
        for ext in format.file_extensions() {
            let ext_lower = ext.to_lowercase();
            self.formats
                .entry(ext_lower)
                .or_default()
                .push(format.clone());
        }
    }

    /// Finds format by extension and optional target.
    ///
    /// Falls back to plugin_formats metadata when no loaded format handles the
    /// extension — returns None in that case since no impl is available, but
    /// logs the discovery so callers can see what's declared.
    pub fn find_by_extension(
        &self,
        extension: &str,
        target: Option<&str>,
    ) -> Option<Arc<dyn FileFormat>> {
        let ext_lower = extension.to_lowercase();

        if let Some(formats) = self.formats.get(&ext_lower) {
            if !formats.is_empty() {
                // If target specified, prefer a matching format.
                if let Some(target_str) = target {
                    let target_token = Token::new(target_str);
                    for format in formats {
                        if format.target() == target_token {
                            return Some(format.clone());
                        }
                    }
                }
                // Return first (primary) loaded format.
                return formats.first().cloned();
            }
        }

        // No loaded format found — check plugin metadata so we can at least
        // log that a plugin declares this extension (no impl to return).
        if !self.plugin_formats.is_empty() {
            let target_token = target.map(Token::new);
            // Primary candidate: is_primary=true (or first match when target given).
            let mut fallback: Option<&PluginFormatInfo> = None;
            for info in &self.plugin_formats {
                if !info.extensions.iter().any(|e| e == &ext_lower) {
                    continue;
                }
                if let Some(ref t) = target_token {
                    if &info.target == t {
                        log::debug!(
                            "find_by_extension: plugin '{}' declares ext '{}' (format '{}') but is not loaded",
                            info.plugin_name,
                            ext_lower,
                            info.format_id
                        );
                        return None;
                    }
                } else if info.is_primary || fallback.is_none() {
                    fallback = Some(info);
                }
            }
            if let Some(info) = fallback {
                log::debug!(
                    "find_by_extension: plugin '{}' declares ext '{}' (format '{}') but is not loaded",
                    info.plugin_name,
                    ext_lower,
                    info.format_id
                );
            }
        }

        None
    }

    /// Finds format by format ID.
    ///
    /// Also checks plugin_formats metadata (logs if found but not loaded).
    pub fn find_by_id(&self, format_id: &Token) -> Option<Arc<dyn FileFormat>> {
        if let Some(format) = self.formats_by_id.get(format_id) {
            return Some(format.clone());
        }

        // Check plugin metadata.
        if let Some(info) = self
            .plugin_formats
            .iter()
            .find(|i| &i.format_id == format_id)
        {
            log::debug!(
                "find_by_id: plugin '{}' declares format '{}' but it is not loaded",
                info.plugin_name,
                format_id
            );
        }

        None
    }

    /// Returns all registered formats.
    pub fn all_formats(&self) -> Vec<Arc<dyn FileFormat>> {
        self.formats_by_id.values().cloned().collect()
    }

    /// Returns all file extensions.
    pub fn all_extensions(&self) -> Vec<String> {
        self.formats.keys().cloned().collect()
    }
}

/// Global format registry instance.
static FORMAT_REGISTRY: std::sync::LazyLock<RwLock<FileFormatRegistry>> =
    std::sync::LazyLock::new(|| RwLock::new(FileFormatRegistry::new()));

/// One-time flag: plugin discovery has been run against PlugRegistry.
static PLUGINS_DISCOVERED: OnceLock<()> = OnceLock::new();

/// Runs plugin discovery exactly once, writing discovered metadata into the registry.
fn ensure_plugins_discovered() {
    PLUGINS_DISCOVERED.get_or_init(|| {
        let mut registry = FORMAT_REGISTRY
            .write()
            .expect("FileFormat registry lock poisoned");
        registry.discover_format_plugins();
    });
}

// ============================================================================
// Public Registry Functions
// ============================================================================

/// Returns a reference to the global file format registry.
///
/// This provides type-safe access to the registry for advanced use cases.
/// For common operations, prefer the module-level functions like
/// [`find_format_by_extension`] and [`register_file_format`].
pub fn get_format_registry() -> &'static RwLock<FileFormatRegistry> {
    &FORMAT_REGISTRY
}

/// Registers a file format globally.
///
/// After registration, the format will be available via
/// [`find_format_by_extension`] and [`find_format_by_id`].
///
/// # Arguments
///
/// * `format` - Format implementation to register
///
/// # Examples
///
/// ```ignore
/// use usd_sdf::{register_file_format, FileFormat};
/// use std::sync::Arc;
///
/// let format: Arc<dyn FileFormat> = Arc::new(MyFormat);
/// register_file_format(format);
/// ```
pub fn register_file_format(format: Arc<dyn FileFormat>) {
    let mut registry = FORMAT_REGISTRY
        .write()
        .expect("FileFormat registry lock poisoned");
    registry.register(format);
}

/// Finds a file format by extension and optional target.
///
/// If multiple formats handle the same extension, the target
/// parameter can be used to select a specific one. If no target
/// is provided, the primary (first registered) format is returned.
///
/// # Arguments
///
/// * `extension` - File extension without leading dot (e.g., "usda")
/// * `target` - Optional target schema name (e.g., "usdGeom")
///
/// # Returns
///
/// The matching format, or None if no format handles the extension.
///
/// # Examples
///
/// ```ignore
/// use usd_sdf::find_format_by_extension;
///
/// // Find any format for .usda
/// let format = find_format_by_extension("usda", None);
///
/// // Find format for .usd with usdGeom target
/// let format = find_format_by_extension("usd", Some("usdGeom"));
/// ```
#[must_use]
pub fn find_format_by_extension(
    extension: &str,
    target: Option<&str>,
) -> Option<Arc<dyn FileFormat>> {
    ensure_plugins_discovered();
    let registry = FORMAT_REGISTRY
        .read()
        .expect("FileFormat registry lock poisoned");
    registry.find_by_extension(extension, target)
}

/// Finds a file format by its unique identifier.
///
/// # Arguments
///
/// * `format_id` - Format identifier (e.g., "usda", "usdc")
///
/// # Returns
///
/// The matching format, or None if not found.
///
/// # Examples
///
/// ```ignore
/// use usd_sdf::find_format_by_id;
/// use usd_tf::Token;
///
/// let format_id = Token::new("usda");
/// let format = find_format_by_id(&format_id);
/// ```
#[must_use]
pub fn find_format_by_id(format_id: &Token) -> Option<Arc<dyn FileFormat>> {
    ensure_plugins_discovered();
    let registry = FORMAT_REGISTRY
        .read()
        .expect("FileFormat registry lock poisoned");
    registry.find_by_id(format_id)
}

/// Returns all registered file formats.
///
/// # Examples
///
/// ```ignore
/// use usd_sdf::get_all_formats;
///
/// let formats = get_all_formats();
/// for format in formats {
///     println!("Format: {} ({})",
///              format.format_id(),
///              format.primary_file_extension());
/// }
/// ```
#[must_use]
pub fn get_all_formats() -> Vec<Arc<dyn FileFormat>> {
    let registry = FORMAT_REGISTRY
        .read()
        .expect("FileFormat registry lock poisoned");
    registry.all_formats()
}

/// Returns all registered file extensions.
///
/// # Examples
///
/// ```ignore
/// use usd_sdf::get_all_file_extensions;
///
/// let extensions = get_all_file_extensions();
/// assert!(extensions.contains(&"usda".to_string()));
/// ```
#[must_use]
pub fn get_all_file_extensions() -> Vec<String> {
    let registry = FORMAT_REGISTRY
        .read()
        .expect("FileFormat registry lock poisoned");
    registry.all_extensions()
}

/// Extracts the file extension from a path.
///
/// Returns the extension without the leading dot.
///
/// # Arguments
///
/// * `path` - File path
///
/// # Examples
///
/// ```ignore
/// use usd_sdf::get_file_extension;
///
/// assert_eq!(get_file_extension("/path/to/file.usda"), Some("usda".to_string()));
/// assert_eq!(get_file_extension("/path/to/file"), None);
/// ```
#[must_use]
pub fn get_file_extension(path: &str) -> Option<String> {
    std::path::Path::new(path)
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|s| s.to_lowercase())
}

/// Checks if the file format for the given asset path is a dynamic file format.
///
/// Dynamic file formats compute their arguments from composed field values
/// at runtime. This is used during payload evaluation to determine whether
/// a payload should be processed as dynamic or static.
///
/// # Arguments
///
/// * `asset_path` - Path to the asset file
/// * `target` - Optional file format target
///
/// # Returns
///
/// `true` if the file format is dynamic, `false` otherwise.
#[must_use]
pub fn is_dynamic_file_format(asset_path: &str, target: Option<&str>) -> bool {
    if asset_path.is_empty() {
        return false;
    }

    // Get file extension
    let extension = match get_file_extension(asset_path) {
        Some(ext) => ext,
        None => return false,
    };

    // Find the file format for this extension
    match find_format_by_extension(&extension, target) {
        Some(format) => format.is_dynamic(),
        None => false,
    }
}

/// Returns the dynamic file format for the given asset path, if any.
///
/// # Arguments
///
/// * `asset_path` - Path to the asset file
/// * `target` - Optional file format target
///
/// # Returns
///
/// The file format if it's dynamic, None otherwise.
#[must_use]
pub fn get_dynamic_file_format(
    asset_path: &str,
    target: Option<&str>,
) -> Option<Arc<dyn FileFormat>> {
    if asset_path.is_empty() {
        return None;
    }

    // Get file extension
    let extension = get_file_extension(asset_path)?;

    // Find the file format for this extension
    let format = find_format_by_extension(&extension, target)?;

    if format.is_dynamic() {
        Some(format)
    } else {
        None
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // Mock format for testing
    struct TestFormat {
        id: Token,
        target: Token,
        extensions: Vec<String>,
    }

    impl FileFormat for TestFormat {
        fn format_id(&self) -> Token {
            self.id.clone()
        }

        fn target(&self) -> Token {
            self.target.clone()
        }

        fn file_extensions(&self) -> Vec<String> {
            self.extensions.clone()
        }

        fn can_read(&self, path: &str) -> bool {
            self.extensions
                .iter()
                .any(|ext| path.ends_with(&format!(".{}", ext)))
        }

        fn read(
            &self,
            _layer: &mut Layer,
            _resolved_path: &ResolvedPath,
            _metadata_only: bool,
        ) -> Result<(), FileFormatError> {
            Ok(())
        }
    }

    #[test]
    fn test_file_format_arguments() {
        let mut args = FileFormatArguments::new();
        assert!(args.is_empty());

        args.insert("target", "usd");
        assert_eq!(args.len(), 1);
        assert_eq!(args.get("target"), Some(&"usd".to_string()));
        assert_eq!(args.target(), Some("usd"));

        args.set_target("usdGeom");
        assert_eq!(args.target(), Some("usdGeom"));

        args.remove("target");
        assert!(args.is_empty());
    }

    #[test]
    fn test_file_format_error_display() {
        let err = FileFormatError::read_error("/path/to/file.usda", "File not found");
        assert!(err.to_string().contains("Failed to read"));
        assert!(err.to_string().contains("file.usda"));

        let err = FileFormatError::unsupported_format("xyz", "Unknown format");
        assert!(err.to_string().contains("Unsupported"));
        assert!(err.to_string().contains("xyz"));

        let err = FileFormatError::version_mismatch("2.0", "1.0");
        assert!(err.to_string().contains("Version mismatch"));
    }

    #[test]
    fn test_get_file_extension() {
        assert_eq!(
            get_file_extension("/path/to/file.usda"),
            Some("usda".to_string())
        );
        assert_eq!(
            get_file_extension("/path/to/file.USDA"),
            Some("usda".to_string())
        );
        assert_eq!(get_file_extension("/path/to/file"), None);
        assert_eq!(get_file_extension("file.usd"), Some("usd".to_string()));
    }

    #[test]
    fn test_format_trait_defaults() {
        let format = TestFormat {
            id: Token::new("test"),
            target: Token::new("usd"),
            extensions: vec!["test".to_string(), "tst".to_string()],
        };

        assert_eq!(format.primary_file_extension(), "test");
        assert!(format.is_supported_extension("test"));
        assert!(format.is_supported_extension("tst"));
        assert!(format.is_supported_extension("TEST")); // Case insensitive
        assert!(!format.is_supported_extension("other"));

        assert!(!format.is_package());
        assert_eq!(
            format.get_package_root_layer_path(&ResolvedPath::empty()),
            ""
        );
        assert!(format.supports_reading());
        assert!(!format.supports_writing());
        assert!(!format.supports_editing());

        let args = format.get_file_format_arguments("/path/to/file.test");
        assert!(args.is_empty());

        let default_args = format.get_default_file_format_arguments();
        assert!(default_args.is_empty());
    }

    #[test]
    fn test_format_read_capability() {
        let format = TestFormat {
            id: Token::new("test"),
            target: Token::new("usd"),
            extensions: vec!["test".to_string()],
        };

        assert!(format.can_read("/path/to/file.test"));
        assert!(!format.can_read("/path/to/file.other"));
        assert!(format.can_write("/path/to/file.test"));
    }

    #[test]
    fn test_format_arguments_conversion() {
        let mut map = HashMap::new();
        map.insert("key1".to_string(), "value1".to_string());
        map.insert("key2".to_string(), "value2".to_string());

        let args: FileFormatArguments = map.clone().into();
        assert_eq!(args.len(), 2);
        assert_eq!(args.get("key1"), Some(&"value1".to_string()));

        let back_to_map: HashMap<String, String> = args.into();
        assert_eq!(back_to_map, map);
    }

    #[test]
    fn test_format_arguments_iteration() {
        let mut args = FileFormatArguments::new();
        args.insert("a", "1");
        args.insert("b", "2");
        args.insert("c", "3");

        let count = args.iter().count();
        assert_eq!(count, 3);

        args.clear();
        assert!(args.is_empty());
    }
}
