//! USD auto-detect file format (.usd) implementation.
//!
//! The `.usd` extension is a "universal" format that can be either text (usda)
//! or binary (usdc). On read, the format auto-detects the underlying format
//! by trying usdc first (most common), then falling back to usda.
//!
//! On write, the format delegates to the underlying format chosen by:
//! 1. Explicit `format` argument ("usda" or "usdc")
//! 2. The underlying format of the existing data
//! 3. Default: usdc
//!
//! # Examples
//!
//! ```ignore
//! use usd_sdf::Layer;
//!
//! // Opens a .usd file - auto-detects whether it's text or binary
//! let layer = Layer::find_or_open("model.usd").unwrap();
//! ```

use std::sync::Arc;

use usd_ar::ResolvedPath;
use usd_tf::Token;

use super::Layer;
use super::file_format::{
    FileFormat, FileFormatArguments, FileFormatError, find_format_by_id, register_file_format,
};

// ============================================================================
// Constants
// ============================================================================

/// Default underlying format for new .usd files (usdc = binary).
const DEFAULT_FORMAT: &str = "usdc";

/// Format argument key for specifying underlying format.
const FORMAT_ARG_KEY: &str = "format";

// ============================================================================
// UsdFileFormat
// ============================================================================

/// The `.usd` auto-detect file format.
///
/// Delegates to usda or usdc depending on file content (read) or
/// explicit format argument / default (write).
pub struct UsdFileFormat;

impl UsdFileFormat {
    /// Creates a new UsdFileFormat instance.
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    /// Resolves which underlying format to use for writing.
    ///
    /// Priority: explicit "format" arg > default (usdc).
    fn resolve_write_format(&self, args: &FileFormatArguments) -> Option<Arc<dyn FileFormat>> {
        // Check for explicit format argument
        if let Some(fmt) = args.get(FORMAT_ARG_KEY) {
            if fmt == "usda" || fmt == "usdc" {
                return find_format_by_id(&Token::new(fmt));
            }
            eprintln!(
                "Warning: USD format argument '{}' must be 'usda' or 'usdc', using default '{}'",
                fmt, DEFAULT_FORMAT
            );
        }

        // Fall back to default
        find_format_by_id(&Token::new(DEFAULT_FORMAT))
    }
}

impl Default for UsdFileFormat {
    fn default() -> Self {
        Self::new()
    }
}

impl FileFormat for UsdFileFormat {
    fn format_id(&self) -> Token {
        Token::new("usd")
    }

    fn target(&self) -> Token {
        Token::new("usd")
    }

    fn file_extensions(&self) -> Vec<String> {
        vec!["usd".to_string()]
    }

    fn can_read(&self, path: &str) -> bool {
        let lower = path.to_ascii_lowercase();
        lower.ends_with(".usd")
    }

    fn read(
        &self,
        layer: &mut Layer,
        resolved_path: &ResolvedPath,
        metadata_only: bool,
    ) -> Result<(), FileFormatError> {
        // Try usdc first (most common), then usda
        if let Some(usdc) = find_format_by_id(&Token::new("usdc")) {
            if usdc.can_read(resolved_path.as_str()) {
                if let Ok(()) = usdc.read(layer, resolved_path, metadata_only) {
                    return Ok(());
                }
            }
        }

        if let Some(usda) = find_format_by_id(&Token::new("usda")) {
            if let Ok(()) = usda.read(layer, resolved_path, metadata_only) {
                return Ok(());
            }
        }

        Err(FileFormatError::read_error(
            resolved_path.as_str(),
            "File is neither valid usdc nor usda",
        ))
    }

    fn write_to_file(
        &self,
        layer: &Layer,
        file_path: &str,
        comment: Option<&str>,
        args: &FileFormatArguments,
    ) -> Result<(), FileFormatError> {
        let format = self.resolve_write_format(args).ok_or_else(|| {
            FileFormatError::write_error(file_path, "No underlying format found for .usd")
        })?;
        format.write_to_file(layer, file_path, comment, args)
    }

    fn write_to_string(
        &self,
        layer: &Layer,
        comment: Option<&str>,
    ) -> Result<String, FileFormatError> {
        // Default to usda for string output (human readable)
        let format = find_format_by_id(&Token::new("usda"))
            .ok_or_else(|| FileFormatError::other("usda format not registered"))?;
        format.write_to_string(layer, comment)
    }

    fn supports_reading(&self) -> bool {
        true
    }

    fn supports_writing(&self) -> bool {
        true
    }

    fn supports_editing(&self) -> bool {
        true
    }

    fn get_version_string(&self) -> Token {
        Token::new("1.0")
    }
}

// ============================================================================
// Registration
// ============================================================================

/// Registers the `.usd` auto-detect file format.
pub fn register_usd_format() {
    register_file_format(Arc::new(UsdFileFormat::new()));
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_id() {
        let fmt = UsdFileFormat::new();
        assert_eq!(fmt.format_id(), Token::new("usd"));
    }

    #[test]
    fn test_extensions() {
        let fmt = UsdFileFormat::new();
        assert!(fmt.file_extensions().contains(&"usd".to_string()));
        assert!(fmt.is_supported_extension("usd"));
    }

    #[test]
    fn test_can_read() {
        let fmt = UsdFileFormat::new();
        assert!(fmt.can_read("model.usd"));
        assert!(fmt.can_read("scene.USD"));
        assert!(!fmt.can_read("model.usda"));
        assert!(!fmt.can_read("model.usdc"));
    }

    #[test]
    fn test_supports() {
        let fmt = UsdFileFormat::new();
        assert!(fmt.supports_reading());
        assert!(fmt.supports_writing());
        assert!(fmt.supports_editing());
    }
}
