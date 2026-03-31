//! Alembic file format (.abc) implementation.
//!
//! This module provides the file format handler for Alembic files.
//! The `.abc` format allows reading Alembic archives as USD layers.
//!
//! # Porting Status
//!
//! This is a port of `pxr/usd/plugin/usdAbc/alembicFileFormat.{cpp,h}`.
//! The functionality is integrated as a static file format (not a plugin).
//!
//! # Dependencies
//!
//! Requires Alembic library for reading/writing .abc files.
//! Options:
//! - Use Rust bindings for Alembic (if available)
//! - Port Alembic reading/writing functionality directly
//! - Use FFI to call Alembic C++ library
//!
//! # File Structure
//!
//! Alembic files use a hierarchical structure similar to USD:
//! - Objects (prims)
//! - Properties (attributes/relationships)
//! - Time samples for animation
//!
//! # Examples
//!
//! ```ignore
//! use usd_sdf::{Layer, find_format_by_extension};
//!
//! let format = find_format_by_extension("abc", None).unwrap();
//! let layer = Layer::find_or_open("model.abc").unwrap();
//! ```

use std::sync::Arc;
use usd_ar::ResolvedPath;
use usd_tf::Token;

use super::Layer;
use super::file_format::{
    FileFormat, FileFormatArguments, FileFormatError, find_format_by_id, register_file_format,
};

// Re-export for convenience
pub use super::abc_data::AlembicData;

// ============================================================================
// Tokens
// ============================================================================

/// Tokens for abc file format.
pub mod tokens {
    use std::sync::OnceLock;
    use usd_tf::Token;

    /// Format ID token
    pub fn id() -> Token {
        static TOKEN: OnceLock<Token> = OnceLock::new();
        TOKEN.get_or_init(|| Token::new("abc")).clone()
    }

    /// Version token
    pub fn version() -> Token {
        static TOKEN: OnceLock<Token> = OnceLock::new();
        TOKEN.get_or_init(|| Token::new("1.0")).clone()
    }

    /// Target token
    pub fn target() -> Token {
        static TOKEN: OnceLock<Token> = OnceLock::new();
        TOKEN.get_or_init(|| Token::new("usd")).clone()
    }
}

// ============================================================================
// AlembicFileFormat
// ============================================================================

/// File format handler for Alembic (.abc) files.
///
/// Matches C++ `UsdAbcAlembicFileFormat`.
pub struct AlembicFileFormat {
    /// Reference to usda format for string operations
    usda_format: Option<Arc<dyn FileFormat>>,
}

impl AlembicFileFormat {
    /// Creates a new Alembic file format.
    pub fn new() -> Self {
        // Get usda format for string operations (ReadFromString, WriteToString, WriteToStream)
        let usda_format = find_format_by_id(&Token::new("usda"));

        Self { usda_format }
    }
}

impl Default for AlembicFileFormat {
    fn default() -> Self {
        Self::new()
    }
}

impl FileFormat for AlembicFileFormat {
    fn format_id(&self) -> Token {
        tokens::id()
    }

    fn target(&self) -> Token {
        tokens::target()
    }

    fn file_extensions(&self) -> Vec<String> {
        vec!["abc".to_string()]
    }

    fn can_read(&self, path: &str) -> bool {
        // Check file extension
        if let Some(ext) = std::path::Path::new(path).extension() {
            if ext == "abc" {
                // Note: Could verify Alembic file header magic bytes.
                // C++ code checks extension and mentions "XXX: Add more verification of file header magic"
                return true;
            }
        }
        false
    }

    fn read(
        &self,
        layer: &mut Layer,
        resolved_path: &ResolvedPath,
        _metadata_only: bool,
    ) -> Result<(), FileFormatError> {
        // Create AlembicData and open the file
        let args = FileFormatArguments::new(); // Note: Would get from layer metadata.
        let mut abc_data = AlembicData::new(args);

        let path_str = resolved_path.as_str();
        if !abc_data.open(path_str) {
            return Err(FileFormatError::read_error(
                path_str,
                abc_data
                    .get_errors()
                    .unwrap_or_else(|| "Failed to open Alembic file".to_string()),
            ));
        }

        // Set the layer data - replace the data in the layer's RwLock
        // Note: Layer.data is pub(crate), so we can access it directly
        let mut data_guard = layer.data.write().expect("rwlock poisoned");
        *data_guard = Box::new(abc_data);
        Ok(())
    }

    fn write_to_file(
        &self,
        layer: &Layer,
        file_path: &str,
        comment: Option<&str>,
        _args: &FileFormatArguments,
    ) -> Result<(), FileFormatError> {
        // Get the layer data
        let _data_guard = layer.data.read().expect("rwlock poisoned");

        // Write using AlembicData::write
        // Note: We need to convert Box<dyn AbstractData> to Arc<dyn AbstractData>
        // For now, this is a placeholder - full implementation requires proper Arc conversion
        let _comment_str = comment.unwrap_or("");

        // Note: Requires Box->Arc conversion for write; Layer stores Box.
        // For Alembic writing, we need to access the data as Arc
        // This is a limitation - AlembicData::write expects Arc, but Layer stores Box
        // We'll need to refactor or create a wrapper

        Err(FileFormatError::write_error(
            file_path,
            "Alembic file writing not yet implemented - requires Alembic library integration"
                .to_string(),
        ))
    }

    fn write_to_string(
        &self,
        layer: &Layer,
        comment: Option<&str>,
    ) -> Result<String, FileFormatError> {
        // XXX: For now, defer to the usda file format for this.
        // C++ code does the same: "XXX: For now, defer to the usda file format for this."
        if let Some(ref usda_format) = self.usda_format {
            return usda_format.write_to_string(layer, comment);
        }

        Err(FileFormatError::other(
            "String serialization not supported for Alembic format",
        ))
    }
}

// ============================================================================
// Registration
// ============================================================================

/// Registers the Alembic file format globally.
///
/// This should be called during library initialization.
pub fn register_abc_format() {
    let format: Arc<dyn FileFormat> = Arc::new(AlembicFileFormat::new());
    register_file_format(format);
}
