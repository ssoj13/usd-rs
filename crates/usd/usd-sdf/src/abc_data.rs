//! Alembic data storage implementation.
//!
//! This module provides `AlembicData`, the `AbstractData` adapter used when an
//! existing Alembic archive is opened as an Sdf layer.
//!
//! Important: this adapter is read-only. Mutating `AbstractData` calls such as
//! `set_field()` are not how Alembic authoring works here. Writing uses the
//! separate `AlembicDataWriter` export path instead of editing an opened `.abc`
//! layer in place.
//!
//! # Porting Status
//!
//! This is a port of `pxr/usd/plugin/usdAbc/alembicData.{cpp,h}`.
//!
//! # Architecture
//!
//! The Alembic translator has several major parts:
//!
//! 1. **Data type translation** - Types and functions for converting between
//!    Alembic and USD data types.
//!
//! 2. **AlembicDataConversion** - Class for holding data type conversion tables.
//!    Converts Alembic properties to USD values and vice versa.
//!
//! 3. **AlembicDataReader** - Backing implementation that acts like a key/value
//!    database backed by Alembic. When an Alembic file is opened, it scans the
//!    object/property hierarchy and caches state for fast lookup.
//!
//! 4. **AlembicDataWriter** - Unlike the reader, the writer does not support the
//!    AbstractData API and we can't use Alembic as an authoring layer. Alembic
//!    is not suitable for interactive editing. This class only supports creating
//!    an Alembic file, dumping a layer to it, and closing the file.
//!
//! 5. **AlembicData** - Forwards most calls to AlembicDataReader while an Alembic
//!    archive is opened as an Sdf layer. This read path is intentionally
//!    immutable; authoring goes through `AlembicDataWriter`, not through
//!    in-place `AbstractData` mutations on an opened archive.

use std::sync::{Arc, RwLock};
use usd_tf::Token;
use usd_vt::Value;

use super::abstract_data::AbstractData;
use super::file_format::FileFormatArguments;
use super::path::Path;
use super::types::{SpecType, TimeSamples};

// Re-export reader and writer
pub use super::abc_reader::AlembicDataReader;
pub use super::abc_writer::AlembicDataWriter;

// ============================================================================
// AlembicData
// ============================================================================

/// Provides an AbstractData interface to Alembic data.
///
/// Matches C++ `UsdAbc_AlembicData`.
///
/// # Examples
///
/// ```ignore
/// use usd_sdf::abc_data::AlembicData;
///
/// let mut data = AlembicData::new(FileFormatArguments::new());
/// if data.open("model.abc") {
///     // Use data as AbstractData
///     let spec_type = data.get_spec_type(&Path::absolute_root());
/// }
/// ```
pub struct AlembicData {
    /// Reader for Alembic file (exists between Open() and Close())
    reader: Option<Arc<RwLock<AlembicDataReader>>>,
    /// File format arguments
    arguments: FileFormatArguments,
    /// Error messages
    errors: Arc<RwLock<Vec<String>>>,
}

impl AlembicData {
    /// Creates a new AlembicData object.
    ///
    /// Outside a successful Open() and Close() pairing, the data acts as if
    /// it contains a pseudo-root prim spec at the absolute root path.
    ///
    /// Matches C++ `UsdAbc_AlembicData::New()`.
    pub fn new(arguments: FileFormatArguments) -> Self {
        Self {
            reader: None,
            arguments,
            errors: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Opens the Alembic file at file_path read-only (closing any open file).
    ///
    /// Alembic is not meant to be used as an in-memory store for editing so
    /// methods that modify the file are not supported.
    ///
    /// Matches C++ `UsdAbc_AlembicData::Open()`.
    pub fn open(&mut self, file_path: &str) -> bool {
        // Close any existing reader
        self.close();

        // Create new reader
        let mut reader = AlembicDataReader::new();

        // Set flags based on environment variables
        // These match the C++ env vars in pxr/usd/plugin/usdAbc/alembicReader.cpp
        if std::env::var("USD_ABC_EXPAND_INSTANCES").is_ok() {
            reader.set_flag(Token::new("expandInstances"), true);
        }
        if std::env::var("USD_ABC_DISABLE_INSTANCING").is_ok() {
            reader.set_flag(Token::new("disableInstancing"), true);
        }
        if std::env::var("USD_ABC_PARENT_INSTANCES").is_ok() {
            reader.set_flag(Token::new("promoteInstances"), true);
        }

        // Open the archive
        if reader.open(file_path, &self.arguments) {
            self.reader = Some(Arc::new(RwLock::new(reader)));
            return true;
        }

        // Store errors
        if let Some(err_msg) = reader.get_errors() {
            self.errors.write().expect("rwlock poisoned").push(err_msg);
        }
        false
    }

    /// Closes the Alembic file.
    ///
    /// This does nothing if already closed. After the call it's as if the
    /// object was just created by New().
    ///
    /// Matches C++ `UsdAbc_AlembicData::Close()`.
    pub fn close(&mut self) {
        self.reader = None;
    }

    /// Write the contents of data to a new or truncated Alembic file at
    /// file_path with the comment.
    ///
    /// Matches C++ `UsdAbc_AlembicData::Write()`.
    pub fn write(_data: &Arc<dyn AbstractData>, _file_path: &str, _comment: &str) -> bool {
        // Note: Requires Alembic C++ library (Abc::OArchive) via FFI.
        // Would need: Alembic lib integration, USD→Abc type conversion, archive I/O.
        // Returns false until Alembic bindings are available.
        false
    }

    /// Returns error messages if any.
    pub fn get_errors(&self) -> Option<String> {
        let errors = self.errors.read().expect("rwlock poisoned");
        if errors.is_empty() {
            None
        } else {
            Some(errors.join("; "))
        }
    }
}

impl AbstractData for AlembicData {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn streams_data(&self) -> bool {
        true
    }

    fn is_empty(&self) -> bool {
        if let Some(ref reader) = self.reader {
            // Check if reader has any specs (besides pseudo-root)
            let reader_guard = reader.read().expect("rwlock poisoned");
            !reader_guard.has_spec(&Path::absolute_root()) || reader_guard.list_specs().len() <= 1
        } else {
            // No reader - only pseudo-root exists
            true
        }
    }

    fn create_spec(&mut self, _path: &Path, _spec_type: SpecType) {
        // Alembic files are read-only; log a warning instead of crashing (mirrors C++ TF_CODING_ERROR).
        eprintln!("[usd-sdf] AlembicData: CreateSpec() not supported - Alembic is read-only");
    }

    fn has_spec(&self, path: &Path) -> bool {
        if let Some(ref reader) = self.reader {
            reader.read().expect("rwlock poisoned").has_spec(path)
        } else {
            // No reader - only pseudo-root exists
            path == &Path::absolute_root()
        }
    }

    fn erase_spec(&mut self, _path: &Path) {
        // Alembic files are read-only; log a warning instead of crashing (mirrors C++ TF_CODING_ERROR).
        eprintln!("[usd-sdf] AlembicData: EraseSpec() not supported - Alembic is read-only");
    }

    fn move_spec(&mut self, _old_path: &Path, _new_path: &Path) {
        // Alembic files are read-only; log a warning instead of crashing (mirrors C++ TF_CODING_ERROR).
        eprintln!("[usd-sdf] AlembicData: MoveSpec() not supported - Alembic is read-only");
    }

    fn get_spec_type(&self, path: &Path) -> SpecType {
        if let Some(ref reader) = self.reader {
            reader.read().expect("rwlock poisoned").get_spec_type(path)
        } else {
            // No reader - only pseudo-root exists
            if path == &Path::absolute_root() {
                SpecType::PseudoRoot
            } else {
                SpecType::Unknown
            }
        }
    }

    fn visit_specs(&self, visitor: &mut dyn super::abstract_data::SpecVisitor) {
        if let Some(ref reader) = self.reader {
            reader
                .read()
                .expect("rwlock poisoned")
                .visit_specs(self, visitor);
        } else {
            // No reader - nothing to visit
        }
    }

    fn has_field(&self, path: &Path, field_name: &Token) -> bool {
        if let Some(ref reader) = self.reader {
            reader
                .read()
                .expect("rwlock poisoned")
                .has_field(path, field_name)
        } else {
            // No reader - pseudo-root has no fields
            false
        }
    }

    fn get_field(&self, path: &Path, field_name: &Token) -> Option<Value> {
        if let Some(ref reader) = self.reader {
            reader
                .read()
                .expect("rwlock poisoned")
                .get_field(path, field_name)
        } else {
            // No reader - pseudo-root has no field values
            None
        }
    }

    fn set_field(&mut self, _path: &Path, _field_name: &Token, _value: Value) {
        // Opened Alembic-backed Sdf layers are immutable. Authoring must go
        // through the dedicated AlembicDataWriter/export path rather than
        // mutating the reader-backed AbstractData adapter in place.
        eprintln!(
            "[usd-sdf] AlembicData: SetField() not supported on opened .abc layers; use AlembicDataWriter/export path"
        );
    }

    fn erase_field(&mut self, _path: &Path, _field_name: &Token) {
        // Alembic files are read-only; log a warning instead of crashing (mirrors C++ TF_CODING_ERROR).
        eprintln!("[usd-sdf] AlembicData: EraseField() not supported - Alembic is read-only");
    }

    fn list_fields(&self, path: &Path) -> Vec<Token> {
        if let Some(ref reader) = self.reader {
            reader.read().expect("rwlock poisoned").list_fields(path)
        } else {
            // No reader - pseudo-root has no fields
            Vec::new()
        }
    }

    fn list_all_time_samples(&self) -> TimeSamples {
        if let Some(ref reader) = self.reader {
            reader.read().expect("rwlock poisoned").list_time_samples()
        } else {
            TimeSamples::new()
        }
    }

    fn list_time_samples_for_path(&self, path: &Path) -> TimeSamples {
        if let Some(ref reader) = self.reader {
            reader
                .read()
                .expect("rwlock poisoned")
                .list_time_samples_for_path(path)
        } else {
            TimeSamples::new()
        }
    }

    fn get_bracketing_time_samples(&self, time: f64) -> Option<(f64, f64)> {
        if let Some(ref reader) = self.reader {
            reader
                .read()
                .expect("rwlock poisoned")
                .get_bracketing_time_samples(time)
        } else {
            None
        }
    }

    fn get_num_time_samples_for_path(&self, path: &Path) -> usize {
        if let Some(ref reader) = self.reader {
            reader
                .read()
                .expect("rwlock poisoned")
                .get_num_time_samples_for_path(path)
        } else {
            0
        }
    }

    fn get_bracketing_time_samples_for_path(&self, path: &Path, time: f64) -> Option<(f64, f64)> {
        if let Some(ref reader) = self.reader {
            reader
                .read()
                .expect("rwlock poisoned")
                .get_bracketing_time_samples_for_path(path, time)
        } else {
            None
        }
    }

    fn get_previous_time_sample_for_path(&self, path: &Path, time: f64) -> Option<f64> {
        if let Some(ref reader) = self.reader {
            reader
                .read()
                .expect("rwlock poisoned")
                .get_previous_time_sample_for_path(path, time)
        } else {
            None
        }
    }

    fn query_time_sample(&self, path: &Path, time: f64) -> Option<Value> {
        if let Some(ref reader) = self.reader {
            reader
                .read()
                .expect("rwlock poisoned")
                .query_time_sample(path, time)
        } else {
            None
        }
    }

    fn set_time_sample(&mut self, _path: &Path, _time: f64, _value: Value) {
        // Same rationale as set_field(): the opened Alembic layer adapter is a
        // reader view, not an in-place authoring backend.
        eprintln!(
            "[usd-sdf] AlembicData: SetTimeSample() not supported on opened .abc layers; use AlembicDataWriter/export path"
        );
    }

    fn erase_time_sample(&mut self, _path: &Path, _time: f64) {
        // Alembic files are read-only; log a warning instead of crashing (mirrors C++ TF_CODING_ERROR).
        eprintln!("[usd-sdf] AlembicData: EraseTimeSample() not supported - Alembic is read-only");
    }
}
