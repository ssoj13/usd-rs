//! USD Crate (binary) file format (.usdc) implementation.
//!
//! This module provides the file format handler for binary USD files.
//! The `.usdc` format is a compact binary representation optimized for
//! fast loading and efficient storage.
//!
//! # File Structure
//!
//! A `.usdc` file uses a custom binary format called "crate" that includes:
//!
//! - Magic cookie: `PXR-USDC`
//! - Version info
//! - Table of contents
//! - Structural sections (specs, fields, paths)
//! - Token tables
//! - String tables
//! - Compressed data blocks (LZ4/uncompressed)
//!
//! # Features
//!
//! - Fast random access to data
//! - Memory-mapped file support
//! - LZ4 compression for data blocks
//! - Efficient path and token deduplication
//! - Lazy loading of time samples
//! - Detached mode for isolated data
//!
//! # String Operations
//!
//! Note: `ReadFromString`, `WriteToString`, and `WriteToStream` delegate
//! to the usda format, as binary data cannot be meaningfully represented
//! as a string.
//!
//! # Examples
//!
//! ```ignore
//! use usd_sdf::{Layer, find_format_by_extension};
//!
//! let format = find_format_by_extension("usdc", None).unwrap();
//! let layer = Layer::find_or_open("model.usdc").unwrap();
//! ```

use std::io::Write;
use std::sync::Arc;

use usd_ar::ResolvedPath;
use usd_tf::Token;

use super::Layer;
use super::abstract_data::AbstractData;
use super::data::Data;
use super::file_format::{
    FileFormat, FileFormatArguments, FileFormatError, find_format_by_id, register_file_format,
};
use super::path::Path;
use super::types::SpecType;
use super::usda_reader;

// ============================================================================
// Tokens
// ============================================================================

/// Tokens for usdc file format.
pub mod tokens {
    use std::sync::OnceLock;
    use usd_tf::Token;

    /// Format ID token
    pub fn id() -> Token {
        static TOKEN: OnceLock<Token> = OnceLock::new();
        TOKEN.get_or_init(|| Token::new("usdc")).clone()
    }
}

// ============================================================================
// Submodules
// ============================================================================

pub mod reader;
pub mod types;
pub mod writer;

// Private imports of helpers needed in this module's impl blocks.
use types::{FromLeBytes, half_to_f32};

// Re-export all types so existing code using `usdc_file_format::X` still works.
pub use reader::CrateFile;
pub use types::{
    BOOTSTRAP_SIZE, Bootstrap, CrateHeader, CrateSpec, CrateTimeSamples, Field, FieldIndex,
    FieldSetIndex, Index, MIN_READ_VERSION, PathIndex, SECTION_NAME_MAX_LENGTH, SOFTWARE_VERSION,
    Section, SectionType, StringIndex, TableOfContents, TokenIndex, TypeEnum, USDC_MAGIC, ValueRep,
    section_names,
};
pub use writer::CrateWriter;

// ============================================================================
// CrateData - Binary format specific data
// ============================================================================

/// Data storage for usdc format layers.
///
/// This provides efficient binary storage with support for:
/// - Memory-mapped files
/// - Lazy loading
/// - Compressed sections
/// - Detached mode
pub struct CrateData {
    /// Base data storage (when fully loaded)
    data: Data,
    /// Whether this data is detached from file backing
    detached: bool,
    /// Source file path (if file-backed)
    source_path: Option<String>,
    /// Parsed crate file (populated after open_bytes / open).
    /// The actual spec/field/token data lives here when file-backed.
    crate_file: Option<CrateFile>,
}

impl CrateData {
    /// Creates new crate data.
    pub fn new(detached: bool) -> Self {
        let mut data = Data::new();
        // The pseudo-root spec must always exist
        data.create_spec(&Path::absolute_root(), SpecType::PseudoRoot);
        Self {
            data,
            detached,
            source_path: None,
            crate_file: None,
        }
    }

    /// Returns the parsed CrateFile, if this data was opened from bytes/file.
    pub fn crate_file(&self) -> Option<&CrateFile> {
        self.crate_file.as_ref()
    }

    /// Returns whether this data is detached.
    pub fn is_detached(&self) -> bool {
        self.detached
    }

    /// Gets the software version token.
    pub fn software_version_token() -> Token {
        Token::new(&format!(
            "{}.{}.{}",
            SOFTWARE_VERSION.0, SOFTWARE_VERSION.1, SOFTWARE_VERSION.2
        ))
    }

    /// Checks if a file can be read as crate data.
    pub fn can_read(path: &str) -> bool {
        if let Ok(data) = std::fs::read(path) {
            Self::can_read_bytes(&data)
        } else {
            false
        }
    }

    /// Checks if bytes can be read as crate data.
    pub fn can_read_bytes(data: &[u8]) -> bool {
        data.len() >= 8 && &data[0..8] == USDC_MAGIC
    }

    /// Opens a crate file.
    pub fn open(&mut self, path: &str, detached: bool) -> Result<(), FileFormatError> {
        let data =
            std::fs::read(path).map_err(|e| FileFormatError::io_error(path, e.to_string()))?;

        self.open_bytes(&data, path, detached)
    }

    /// Opens crate data from bytes.
    ///
    /// Delegates to `CrateFile::open()` which fully parses the binary layout
    /// (bootstrap, TOC, tokens, strings, paths, specs, fields). The resulting
    /// `CrateFile` is stored in `self.crate_file` and can be accessed via
    /// `crate_file()`. The high-level `self.data` map is NOT populated here;
    /// callers that need the composed `Data` representation should use
    /// `UsdcFileFormat::read_from_file()` which populates the layer directly.
    pub fn open_bytes(
        &mut self,
        data: &[u8],
        path: &str,
        detached: bool,
    ) -> Result<(), FileFormatError> {
        // Delegate full binary parsing to CrateFile.
        let mut cf = CrateFile::open(data, path)?;
        cf.detached = detached;
        self.source_path = Some(path.to_string());
        self.detached = detached;
        self.crate_file = Some(cf);
        Ok(())
    }

    /// Exports data to a file.
    pub fn export(&mut self, path: &str) -> Result<(), FileFormatError> {
        let bytes = self.to_bytes()?;
        std::fs::write(path, bytes).map_err(|e| FileFormatError::io_error(path, e.to_string()))?;
        Ok(())
    }

    /// Saves data to its source file.
    pub fn save(&mut self, path: &str) -> Result<(), FileFormatError> {
        // For save, we potentially update in-place
        // For now, just export
        self.export(path)
    }

    /// Serializes a minimal valid USDC binary byte buffer.
    ///
    /// WARNING: This produces a structurally valid but EMPTY crate file.
    /// It does NOT serialize `self.data` — the `CrateWriter` only supports
    /// population from a `Layer` via `populate_from_layer()`.
    ///
    /// For writing a full layer, use `UsdcFileFormat::write_to_file()` or
    /// `UsdcFileFormat::save_to_file()` which drive `CrateWriter` with
    /// the full layer content.
    pub fn to_bytes(&self) -> Result<Vec<u8>, FileFormatError> {
        if !self.data.is_empty() {
            log::warn!(
                "CrateData::to_bytes() ignores self.data; \
                 use UsdcFileFormat::write_to_file() for full serialization"
            );
        }
        let mut writer = CrateWriter::new(SOFTWARE_VERSION);
        Ok(writer.write())
    }

    /// Copies data from another source.
    pub fn copy_from(&mut self, _source: &dyn AbstractData) {
        // Data copying not implemented yet
    }

    /// Gets the underlying data.
    pub fn data(&self) -> &Data {
        &self.data
    }

    /// Gets the underlying data mutably.
    pub fn data_mut(&mut self) -> &mut Data {
        &mut self.data
    }
}

// ============================================================================
// UsdcFileFormat
// ============================================================================

/// File format handler for USD binary crate files (.usdc).
///
/// This format provides efficient binary storage with fast random access,
/// compression, and memory-mapped file support.
///
/// # String Operations
///
/// String-based operations (`read_from_string`, `write_to_string`, `write_to_stream`)
/// delegate to the usda format, as binary data cannot be meaningfully represented
/// as text.
///
/// # Thread Safety
///
/// This type is `Send + Sync` and can be used from multiple threads.
#[derive(Debug, Clone)]
pub struct UsdcFileFormat {
    /// Format identifier
    format_id: Token,
    /// Version string
    version_string: Token,
    /// Target schema
    target: Token,
    /// File extensions
    extensions: Vec<String>,
}

impl Default for UsdcFileFormat {
    fn default() -> Self {
        Self::new()
    }
}

impl UsdcFileFormat {
    /// Creates a new usdc file format handler.
    #[must_use]
    pub fn new() -> Self {
        Self {
            format_id: tokens::id(),
            version_string: CrateData::software_version_token(),
            target: Token::new("usd"),
            extensions: vec!["usdc".to_string()],
        }
    }

    /// Initializes data for a new layer.
    #[must_use]
    pub fn init_data(&self, _args: &FileFormatArguments) -> Box<CrateData> {
        Box::new(CrateData::new(false))
    }

    /// Initializes detached data for a new layer.
    #[must_use]
    pub fn init_detached_data(&self, _args: &FileFormatArguments) -> Box<CrateData> {
        Box::new(CrateData::new(true))
    }

    /// Internal read helper.
    fn read_helper(
        &self,
        layer: &mut Layer,
        resolved_path: &str,
        _metadata_only: bool,
        _detached: bool,
    ) -> Result<(), FileFormatError> {
        usd_trace::trace_scope!("usdc_read_helper");
        let t0 = std::time::Instant::now();

        // Read file data
        let data = std::fs::read(resolved_path)
            .map_err(|e| FileFormatError::io_error(resolved_path, e.to_string()))?;
        let read_ms = t0.elapsed().as_secs_f64() * 1000.0;

        // Parse crate file
        let t1 = std::time::Instant::now();
        let crate_file = CrateFile::open(&data, resolved_path)?;
        let parse_ms = t1.elapsed().as_secs_f64() * 1000.0;

        // Populate layer from crate file
        let t2 = std::time::Instant::now();
        self.populate_layer_from_crate(layer, &crate_file, &data)?;
        let populate_ms = t2.elapsed().as_secs_f64() * 1000.0;

        eprintln!(
            "[PERF] USDC {}: read={:.1}ms parse={:.1}ms populate={:.1}ms total={:.1}ms ({:.1}MB {} specs)",
            resolved_path,
            read_ms,
            parse_ms,
            populate_ms,
            read_ms + parse_ms + populate_ms,
            data.len() as f64 / 1_048_576.0,
            crate_file.specs.len(),
        );

        Ok(())
    }

    /// Populates a layer from parsed crate file data.
    fn populate_layer_from_crate(
        &self,
        layer: &mut Layer,
        crate_file: &CrateFile,
        file_data: &[u8],
    ) -> Result<(), FileFormatError> {
        let mut unpack_ns: u64 = 0;
        let mut setfield_ns: u64 = 0;
        let mut create_spec_ns: u64 = 0;
        let mut field_count: u64 = 0;
        let mut array_count: u64 = 0;
        let mut compressed_count: u64 = 0;
        let mut inlined_count: u64 = 0;

        // Process each spec from the crate file
        for spec in &crate_file.specs {
            // Get path for this spec
            let path = if let Some(p) = crate_file.get_path(spec.path_index) {
                p.clone()
            } else {
                continue;
            };

            // Create spec in layer (raw: skip property child management,
            // USDC already stores propertyChildren as a field)
            let cs0 = std::time::Instant::now();
            match spec.spec_type {
                SpecType::PseudoRoot => {}
                _ => {
                    layer.create_spec_raw(&path, spec.spec_type);
                }
            }
            create_spec_ns += cs0.elapsed().as_nanos() as u64;

            // Get field set for this spec and apply fields
            let field_set_start = spec.field_set_index.0.value as usize;

            // Iterate through field indices until we hit an invalid terminator
            let mut field_idx = field_set_start;
            while field_idx < crate_file.field_sets.len() {
                let field_index = &crate_file.field_sets[field_idx];
                if !field_index.0.is_valid() {
                    break; // Hit terminator
                }

                // Get field data
                if let Some(field) = crate_file.get_field(*field_index) {
                    // Get field name from token
                    if let Some(token) = crate_file.get_token(field.token_index) {
                        let field_name = token.clone();

                        // Unpack value (handles both inlined and non-inlined)
                        let u0 = std::time::Instant::now();
                        let is_array = field.value_rep.is_array();
                        let is_compressed = field.value_rep.is_compressed();
                        let is_inlined = field.value_rep.is_inlined();
                        let val = self.unpack_value(&field.value_rep, crate_file, file_data);
                        unpack_ns += u0.elapsed().as_nanos() as u64;
                        field_count += 1;
                        if is_array {
                            array_count += 1;
                        }
                        if is_compressed {
                            compressed_count += 1;
                        }
                        if is_inlined {
                            inlined_count += 1;
                        }

                        if let Some(v) = val {
                            let s0 = std::time::Instant::now();
                            if field_name == "timeSamples" {
                                if let Some(ts) = v.downcast::<CrateTimeSamples>() {
                                    for (i, &t) in ts.times.iter().enumerate() {
                                        if let Some(sample_val) = ts.values.get(i) {
                                            layer.set_time_sample(&path, t, sample_val.clone());
                                        }
                                    }
                                } else {
                                    layer.set_field(&path, &field_name, v);
                                }
                            } else {
                                layer.set_field(&path, &field_name, v);
                            }
                            setfield_ns += s0.elapsed().as_nanos() as u64;
                        }
                    }
                }

                field_idx += 1;
            }
        }

        eprintln!(
            "[PERF] populate: spec={:.1}ms unpack={:.1}ms set={:.1}ms | fields={} arr={} compr={} inl={}",
            create_spec_ns as f64 / 1_000_000.0,
            unpack_ns as f64 / 1_000_000.0,
            setfield_ns as f64 / 1_000_000.0,
            field_count,
            array_count,
            compressed_count,
            inlined_count,
        );

        Ok(())
    }

    /// Unpacks a value from ValueRep.
    /// Handles both inlined values and values stored in file.
    pub(crate) fn unpack_value(
        &self,
        rep: &ValueRep,
        crate_file: &CrateFile,
        file_data: &[u8],
    ) -> Option<usd_vt::Value> {
        let payload = rep.get_payload();
        let type_enum = rep.get_type();

        // Handle inlined values first
        if rep.is_inlined() {
            return self.unpack_inlined_value(rep, crate_file);
        }

        // Non-inlined: payload is file offset
        let offset = payload as usize;
        if offset >= file_data.len() {
            return None;
        }

        // Handle arrays
        if rep.is_array() {
            // C++ CrateFile::UnpackArray: payload==0 means empty array
            if payload == 0 {
                return self.make_empty_array(type_enum);
            }
            return self.unpack_array_value(rep, offset, crate_file, file_data);
        }

        // Read value from file at offset
        self.unpack_file_value(type_enum, offset, crate_file, file_data)
    }

    /// Unpacks an inlined value from ValueRep.
    fn unpack_inlined_value(
        &self,
        rep: &ValueRep,
        crate_file: &CrateFile,
    ) -> Option<usd_vt::Value> {
        let payload = rep.get_payload();
        let type_enum = rep.get_type();

        match type_enum {
            TypeEnum::Bool => Some(usd_vt::Value::new(payload != 0)),
            TypeEnum::Int => Some(usd_vt::Value::new(payload as i32)),
            TypeEnum::UInt => Some(usd_vt::Value::new(payload as u32)),
            TypeEnum::Int64 => Some(usd_vt::Value::new(payload as i64)),
            TypeEnum::UInt64 => Some(usd_vt::Value::new(payload)),
            TypeEnum::Float => {
                let bits = payload as u32;
                Some(usd_vt::Value::from_f32(f32::from_bits(bits)))
            }
            TypeEnum::Double => {
                // Inlined double is stored as f32 bits in the payload
                // (crateValueInliners.h: _DecodeInline reads u32 as float, casts to FP)
                let f = f32::from_bits(payload as u32);
                Some(usd_vt::Value::from_f64(f as f64))
            }
            TypeEnum::Token => {
                // Token index stored in payload
                let token_idx = TokenIndex::new(payload as u32);
                crate_file
                    .get_token(token_idx)
                    .map(|t| usd_vt::Value::new(t.clone()))
            }
            TypeEnum::Specifier => {
                // Return actual Specifier enum, not a stringified Token (P1-3)
                let spec = match payload {
                    0 => super::Specifier::Def,
                    1 => super::Specifier::Over,
                    2 => super::Specifier::Class,
                    _ => super::Specifier::Def,
                };
                Some(usd_vt::Value::new(spec))
            }
            TypeEnum::Variability => {
                // Return actual Variability enum, not a stringified Token (P1-3)
                let var = if payload == 0 {
                    super::Variability::Varying
                } else {
                    super::Variability::Uniform
                };
                Some(usd_vt::Value::new(var))
            }
            TypeEnum::String | TypeEnum::AssetPath => {
                // String index stored in payload
                let str_idx = StringIndex::new(payload as u32);
                if let Some(s) = crate_file.get_string(str_idx) {
                    if type_enum == TypeEnum::AssetPath {
                        Some(usd_vt::Value::new(crate::AssetPath::new(s)))
                    } else {
                        Some(usd_vt::Value::new(s.to_string()))
                    }
                } else {
                    None
                }
            }
            TypeEnum::PathExpression => {
                // Path expression string
                let str_idx = StringIndex::new(payload as u32);
                crate_file
                    .get_string(str_idx)
                    .map(|s| usd_vt::Value::new(s.to_string()))
            }
            // Vector types: inlined when ALL components fit exactly as int8_t.
            // C++ _DecodeInline for GfVec: memcpy payload bytes into int8_t[N],
            // then cast each component to the scalar type.
            // Payload layout (little-endian u32):
            //   byte0 = x as i8, byte1 = y as i8, byte2 = z as i8, byte3 = w as i8 (for Vec4)
            TypeEnum::Vec2f => {
                let bytes = (payload as u32).to_le_bytes();
                let x = bytes[0] as i8 as f32;
                let y = bytes[1] as i8 as f32;
                Some(usd_vt::Value::from_no_hash(usd_gf::Vec2f::new(x, y)))
            }
            TypeEnum::Vec3f => {
                let bytes = (payload as u32).to_le_bytes();
                let x = bytes[0] as i8 as f32;
                let y = bytes[1] as i8 as f32;
                let z = bytes[2] as i8 as f32;
                Some(usd_vt::Value::from_no_hash(usd_gf::Vec3f::new(x, y, z)))
            }
            TypeEnum::Vec4f => {
                let bytes = (payload as u32).to_le_bytes();
                let x = bytes[0] as i8 as f32;
                let y = bytes[1] as i8 as f32;
                let z = bytes[2] as i8 as f32;
                let w = bytes[3] as i8 as f32;
                Some(usd_vt::Value::from_no_hash(usd_gf::Vec4f::new(x, y, z, w)))
            }
            TypeEnum::Vec2d => {
                let bytes = (payload as u32).to_le_bytes();
                let x = bytes[0] as i8 as f64;
                let y = bytes[1] as i8 as f64;
                Some(usd_vt::Value::from_no_hash(usd_gf::Vec2d::new(x, y)))
            }
            TypeEnum::Vec3d => {
                let bytes = (payload as u32).to_le_bytes();
                let x = bytes[0] as i8 as f64;
                let y = bytes[1] as i8 as f64;
                let z = bytes[2] as i8 as f64;
                Some(usd_vt::Value::from_no_hash(usd_gf::Vec3d::new(x, y, z)))
            }
            TypeEnum::Vec4d => {
                let bytes = (payload as u32).to_le_bytes();
                let x = bytes[0] as i8 as f64;
                let y = bytes[1] as i8 as f64;
                let z = bytes[2] as i8 as f64;
                let w = bytes[3] as i8 as f64;
                Some(usd_vt::Value::from_no_hash(usd_gf::Vec4d::new(x, y, z, w)))
            }
            // Integer vector types: same int8_t inlining scheme, cast to i32.
            TypeEnum::Vec2i => {
                let bytes = (payload as u32).to_le_bytes();
                let x = bytes[0] as i8 as i32;
                let y = bytes[1] as i8 as i32;
                Some(usd_vt::Value::new(usd_gf::Vec2i::new(x, y)))
            }
            TypeEnum::Vec3i => {
                let bytes = (payload as u32).to_le_bytes();
                let x = bytes[0] as i8 as i32;
                let y = bytes[1] as i8 as i32;
                let z = bytes[2] as i8 as i32;
                Some(usd_vt::Value::new(usd_gf::Vec3i::new(x, y, z)))
            }
            TypeEnum::Vec4i => {
                let bytes = (payload as u32).to_le_bytes();
                let x = bytes[0] as i8 as i32;
                let y = bytes[1] as i8 as i32;
                let z = bytes[2] as i8 as i32;
                let w = bytes[3] as i8 as i32;
                Some(usd_vt::Value::new(usd_gf::Vec4i::new(x, y, z, w)))
            }
            TypeEnum::Half => {
                let bits = payload as u16;
                Some(usd_vt::Value::from_f32(half::f16::from_bits(bits).to_f32()))
            }
            // Matrix types: inlined as diagonal int8_t vector (all off-diagonals are 0).
            // C++ _DecodeInline for GfMatrix: extract int8_t[N] from payload, build
            // diagonal matrix: M[i][i] = diag[i], all other entries = 0.
            TypeEnum::Matrix2d => {
                let bytes = (payload as u32).to_le_bytes();
                let mut m = [[0.0f64; 2]; 2];
                for i in 0..2 {
                    m[i][i] = bytes[i] as i8 as f64;
                }
                Some(usd_vt::Value::from_no_hash(usd_gf::Matrix2d::from_array(m)))
            }
            TypeEnum::Matrix3d => {
                let bytes = (payload as u32).to_le_bytes();
                let mut m = [[0.0f64; 3]; 3];
                for i in 0..3 {
                    m[i][i] = bytes[i] as i8 as f64;
                }
                Some(usd_vt::Value::from_no_hash(usd_gf::Matrix3d::from_array(m)))
            }
            TypeEnum::Matrix4d => {
                let bytes = (payload as u32).to_le_bytes();
                let mut m = [[0.0f64; 4]; 4];
                for i in 0..4 {
                    m[i][i] = bytes[i] as i8 as f64;
                }
                Some(usd_vt::Value::from_no_hash(usd_gf::Matrix4d::from_array(m)))
            }
            TypeEnum::Dictionary => {
                // Empty dictionary inlined with payload=0
                Some(usd_vt::Value::new(usd_vt::Dictionary::new()))
            }
            _ => None, // Unsupported inlined type
        }
    }

    /// Unpacks a value from file data at given offset.
    fn unpack_file_value(
        &self,
        type_enum: TypeEnum,
        offset: usize,
        crate_file: &CrateFile,
        file_data: &[u8],
    ) -> Option<usd_vt::Value> {
        let data = &file_data[offset..];

        match type_enum {
            TypeEnum::Bool => {
                if data.is_empty() {
                    return None;
                }
                Some(usd_vt::Value::new(data[0] != 0))
            }
            TypeEnum::Int => {
                if data.len() < 4 {
                    return None;
                }
                let v = i32::from_le_bytes([data[0], data[1], data[2], data[3]]);
                Some(usd_vt::Value::new(v))
            }
            TypeEnum::UInt => {
                if data.len() < 4 {
                    return None;
                }
                let v = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
                Some(usd_vt::Value::new(v))
            }
            TypeEnum::Int64 => {
                if data.len() < 8 {
                    return None;
                }
                let v = i64::from_le_bytes([
                    data[0], data[1], data[2], data[3], data[4], data[5], data[6], data[7],
                ]);
                Some(usd_vt::Value::new(v))
            }
            TypeEnum::UInt64 => {
                if data.len() < 8 {
                    return None;
                }
                let v = u64::from_le_bytes([
                    data[0], data[1], data[2], data[3], data[4], data[5], data[6], data[7],
                ]);
                Some(usd_vt::Value::new(v))
            }
            TypeEnum::Float | TypeEnum::Half => {
                if data.len() < 4 {
                    return None;
                }
                let bits = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
                Some(usd_vt::Value::from_f32(f32::from_bits(bits)))
            }
            TypeEnum::Double => {
                if data.len() < 8 {
                    return None;
                }
                let bits = u64::from_le_bytes([
                    data[0], data[1], data[2], data[3], data[4], data[5], data[6], data[7],
                ]);
                Some(usd_vt::Value::from_f64(f64::from_bits(bits)))
            }
            TypeEnum::String | TypeEnum::AssetPath => {
                // Non-inlined string: stored as u32 StringIndex (same as inlined path).
                // C++ always inlines strings, so this path is rarely reached.
                if data.len() < 4 {
                    return None;
                }
                let idx = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
                let str_idx = StringIndex::new(idx);
                if let Some(s) = crate_file.get_string(str_idx) {
                    if type_enum == TypeEnum::AssetPath {
                        Some(usd_vt::Value::new(crate::AssetPath::new(s)))
                    } else {
                        Some(usd_vt::Value::new(s.to_string()))
                    }
                } else {
                    None
                }
            }
            TypeEnum::Token => {
                // Token index
                if data.len() < 4 {
                    return None;
                }
                let idx = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
                let token_idx = TokenIndex::new(idx);
                crate_file
                    .get_token(token_idx)
                    .map(|t| usd_vt::Value::new(t.clone()))
            }
            TypeEnum::Vec2d | TypeEnum::Vec2f | TypeEnum::Vec2i | TypeEnum::Vec2h => {
                self.unpack_vec2(type_enum, data)
            }
            TypeEnum::Vec3d | TypeEnum::Vec3f | TypeEnum::Vec3i | TypeEnum::Vec3h => {
                self.unpack_vec3(type_enum, data)
            }
            TypeEnum::Vec4d | TypeEnum::Vec4f | TypeEnum::Vec4i | TypeEnum::Vec4h => {
                self.unpack_vec4(type_enum, data)
            }
            TypeEnum::Quatd | TypeEnum::Quatf | TypeEnum::Quath => {
                self.unpack_quat(type_enum, data)
            }
            TypeEnum::Matrix2d => self.unpack_matrix2d(data),
            TypeEnum::Matrix3d => self.unpack_matrix3d(data),
            TypeEnum::Matrix4d => self.unpack_matrix4d(data),
            TypeEnum::TimeSamples => {
                // TimeSamples need special handling - read times and value offsets
                self.unpack_time_samples(offset, crate_file, file_data)
            }
            // ListOp types
            TypeEnum::TokenListOp => self.unpack_token_list_op(data, crate_file),
            TypeEnum::StringListOp => self.unpack_string_list_op(data, crate_file),
            TypeEnum::PathListOp => self.unpack_path_list_op(data, crate_file),
            TypeEnum::ReferenceListOp => self.unpack_reference_list_op(data, crate_file),
            TypeEnum::IntListOp => self.unpack_int_list_op::<i32>(data),
            TypeEnum::Int64ListOp => self.unpack_int_list_op::<i64>(data),
            TypeEnum::UIntListOp => self.unpack_int_list_op::<u32>(data),
            TypeEnum::UInt64ListOp => self.unpack_int_list_op::<u64>(data),
            TypeEnum::PayloadListOp => self.unpack_payload_list_op(data, crate_file),
            // Vector types
            TypeEnum::PathVector => self.unpack_path_vector(data, crate_file),
            TypeEnum::TokenVector => self.unpack_token_vector(data, crate_file),
            TypeEnum::DoubleVector => self.unpack_double_vector(data),
            TypeEnum::StringVector => self.unpack_string_vector(data, crate_file),
            TypeEnum::Dictionary => self.unpack_dictionary(offset, crate_file, file_data),
            TypeEnum::VariantSelectionMap => self.unpack_variant_selection_map(data, crate_file),
            TypeEnum::LayerOffsetVector => self.unpack_layer_offset_vector(data),
            _ => None,
        }
    }

    /// Unpacks a VtDictionary from file data.
    ///
    /// Binary layout (ReadMap<VtDictionary> in C++):
    ///   u64  count                 -- number of key-value pairs
    ///   per pair:
    ///     u32  StringIndex         -- key string (index into crate string table)
    ///     u64  ValueRep            -- value (may be inlined or point to another offset)
    fn unpack_dictionary(
        &self,
        offset: usize,
        crate_file: &CrateFile,
        file_data: &[u8],
    ) -> Option<usd_vt::Value> {
        let data = &file_data[offset..];

        // Read entry count (u64)
        if data.len() < 8 {
            return None;
        }
        let count = u64::from_le_bytes([
            data[0], data[1], data[2], data[3], data[4], data[5], data[6], data[7],
        ]) as usize;

        let mut dict = usd_vt::Dictionary::new();
        let mut pos = 8usize;

        for _ in 0..count {
            // Read StringIndex (u32) for key
            if pos + 4 > data.len() {
                break;
            }
            let str_idx_raw =
                u32::from_le_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]]);
            pos += 4;

            // Read ValueRep (u64) for value
            if pos + 8 > data.len() {
                break;
            }
            let rep_raw = u64::from_le_bytes([
                data[pos],
                data[pos + 1],
                data[pos + 2],
                data[pos + 3],
                data[pos + 4],
                data[pos + 5],
                data[pos + 6],
                data[pos + 7],
            ]);
            pos += 8;

            // Resolve key string via crate string table
            let key = crate_file
                .get_string(StringIndex::new(str_idx_raw))
                .map(|s| s.to_string())
                .unwrap_or_default();

            // Unpack value recursively
            let rep = ValueRep { data: rep_raw };
            if let Some(value) = self.unpack_value(&rep, crate_file, file_data) {
                dict.insert_value(key, value);
            }
        }

        Some(usd_vt::Value::new(dict))
    }

    /// Unpacks SdfVariantSelectionMap from file data.
    /// C++ ReadMap<SdfVariantSelectionMap>: u64 count + per entry: StringIndex key + StringIndex val.
    fn unpack_variant_selection_map(
        &self,
        data: &[u8],
        crate_file: &CrateFile,
    ) -> Option<usd_vt::Value> {
        if data.len() < 8 {
            return None;
        }
        let count = u64::from_le_bytes([
            data[0], data[1], data[2], data[3], data[4], data[5], data[6], data[7],
        ]) as usize;

        let mut map = std::collections::HashMap::<String, String>::new();
        let mut pos = 8usize;

        for _ in 0..count {
            // Read key StringIndex (u32)
            if pos + 4 > data.len() {
                break;
            }
            let key_idx =
                u32::from_le_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]]);
            pos += 4;
            let key = crate_file
                .get_string(StringIndex::new(key_idx))
                .map(|s| s.to_string())
                .unwrap_or_default();

            // Read value StringIndex (u32)
            if pos + 4 > data.len() {
                break;
            }
            let val_idx =
                u32::from_le_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]]);
            pos += 4;
            let val = crate_file
                .get_string(StringIndex::new(val_idx))
                .map(|s| s.to_string())
                .unwrap_or_default();

            map.insert(key, val);
        }

        Some(usd_vt::Value::from_no_hash(map))
    }

    /// Unpacks SdfLayerOffsetVector from file data.
    /// C++ _Read<vector<SdfLayerOffset>>: u64 count + per entry: f64 offset + f64 scale.
    fn unpack_layer_offset_vector(&self, data: &[u8]) -> Option<usd_vt::Value> {
        if data.len() < 8 {
            return None;
        }
        let count = u64::from_le_bytes([
            data[0], data[1], data[2], data[3], data[4], data[5], data[6], data[7],
        ]) as usize;

        let mut offsets = Vec::with_capacity(count);
        let mut pos = 8usize;

        for _ in 0..count {
            if pos + 16 > data.len() {
                break;
            }
            let offset_val = f64::from_bits(u64::from_le_bytes([
                data[pos],
                data[pos + 1],
                data[pos + 2],
                data[pos + 3],
                data[pos + 4],
                data[pos + 5],
                data[pos + 6],
                data[pos + 7],
            ]));
            pos += 8;
            let scale = f64::from_bits(u64::from_le_bytes([
                data[pos],
                data[pos + 1],
                data[pos + 2],
                data[pos + 3],
                data[pos + 4],
                data[pos + 5],
                data[pos + 6],
                data[pos + 7],
            ]));
            pos += 8;
            offsets.push(super::LayerOffset::new(offset_val, scale));
        }

        Some(usd_vt::Value::new(offsets))
    }

    /// Unpacks time samples from file data.
    ///
    /// C++ on-disk layout (from `Write(TimeSamples)` + `_RecursiveWrite`):
    ///   int64  jump1          — forward skip to timesRep (= size of times blob)
    ///   [times blob]          — raw array data (pointed to by timesRep.payload)
    ///   ValueRep timesRep     — 8-byte ref to the times blob
    ///   int64  jump2          — forward skip to numValues (= size of value blobs)
    ///   [value blobs]         — per-sample raw data (pointed to by sample ValueReps)
    ///   uint64 numValues      — sample count
    ///   ValueRep[numValues]   — per-sample references (8 bytes each)
    fn unpack_time_samples(
        &self,
        offset: usize,
        crate_file: &CrateFile,
        file_data: &[u8],
    ) -> Option<usd_vt::Value> {
        if offset + 8 > file_data.len() {
            return None;
        }

        // Step 1: read jump1 and seek past the times blob to timesRep.
        let jump1 = i64::from_le_bytes(file_data[offset..offset + 8].try_into().ok()?) as usize;
        // C++ convention: jump = end - start_of_jump_field (includes 8-byte field)
        let times_rep_offset = offset + jump1;
        if times_rep_offset + 8 > file_data.len() {
            return None;
        }
        let times_rep = ValueRep::from_bytes(&file_data[times_rep_offset..])?;

        // Step 2: decode the times array (timesRep.payload is absolute file offset).
        let times = self.read_times_array(&times_rep, crate_file, file_data)?;

        // Step 3: read jump2 and seek past the value blobs to numValues.
        let jump2_pos = times_rep_offset + 8;
        if jump2_pos + 8 > file_data.len() {
            return None;
        }
        let jump2 =
            i64::from_le_bytes(file_data[jump2_pos..jump2_pos + 8].try_into().ok()?) as usize;
        // C++ convention: jump = end - start_of_jump_field (includes 8-byte field)
        let num_values_pos = jump2_pos + jump2;
        if num_values_pos + 8 > file_data.len() {
            return None;
        }

        // Step 4: numValues followed immediately by the ValueRep array.
        let num_values = u64::from_le_bytes(
            file_data[num_values_pos..num_values_pos + 8]
                .try_into()
                .ok()?,
        ) as usize;
        let values_offset = num_values_pos + 8; // start of ValueRep[numValues]

        // Create CrateTimeSamples with in-file location for lazy loading.
        let mut ts = CrateTimeSamples::new();
        ts.times = times;
        ts.values_file_offset = values_offset as i64;

        // Eagerly decode all sample values (lazy threshold removed — animated scenes
        // can have thousands of samples; silent truncation caused missing animation).
        ts.values = self.read_time_sample_values(values_offset, num_values, crate_file, file_data);

        Some(usd_vt::Value::from_no_hash(ts))
    }

    /// Reads times array from a timesRep ValueRep.
    ///
    /// In the USDC format, the times inside a TimeSamples block are stored as a
    /// `DoubleVector` (type=48) — NOT a VtArray — so its ValueRep does NOT have
    /// IS_ARRAY_BIT set.  The on-disk layout is simply:
    ///   uint64_t count     -- number of double values
    ///   double[count]      -- packed f64 values (little-endian)
    ///
    /// The older VtArray path (with versioned headers) only applies to regular
    /// array fields, not to the times blob inside TimeSamples.
    fn read_times_array(
        &self,
        rep: &ValueRep,
        _crate_file: &CrateFile,
        file_data: &[u8],
    ) -> Option<Vec<f64>> {
        // timesRep is type=DoubleVector, NOT flagged as array.
        // payload is an absolute byte offset into file_data.
        let offset = rep.get_payload() as usize;
        if offset + 8 > file_data.len() {
            return None;
        }

        let data = &file_data[offset..];

        // uint64_t count followed by count * 8 bytes of f64 data.
        let num_elements = u64::from_le_bytes([
            data[0], data[1], data[2], data[3], data[4], data[5], data[6], data[7],
        ]) as usize;

        let samples_start = 8;
        if data.len() < samples_start + num_elements * 8 {
            return None;
        }

        let array_data = &data[samples_start..];
        let mut times = Vec::with_capacity(num_elements);
        for i in 0..num_elements {
            let off = i * 8;
            let bits = u64::from_le_bytes([
                array_data[off],
                array_data[off + 1],
                array_data[off + 2],
                array_data[off + 3],
                array_data[off + 4],
                array_data[off + 5],
                array_data[off + 6],
                array_data[off + 7],
            ]);
            times.push(f64::from_bits(bits));
        }

        Some(times)
    }

    /// Reads time sample values.
    fn read_time_sample_values(
        &self,
        offset: usize,
        num_values: usize,
        crate_file: &CrateFile,
        file_data: &[u8],
    ) -> Vec<usd_vt::Value> {
        let mut values = Vec::with_capacity(num_values);

        for i in 0..num_values {
            let rep_offset = offset + i * 8;
            if rep_offset + 8 > file_data.len() {
                break;
            }

            if let Some(rep) = ValueRep::from_bytes(&file_data[rep_offset..]) {
                if let Some(val) = self.unpack_value(&rep, crate_file, file_data) {
                    values.push(val);
                } else {
                    values.push(usd_vt::Value::empty());
                }
            } else {
                values.push(usd_vt::Value::empty());
            }
        }

        values
    }

    /// Unpacks Vec2 from file data.
    fn unpack_vec2(&self, type_enum: TypeEnum, data: &[u8]) -> Option<usd_vt::Value> {
        match type_enum {
            TypeEnum::Vec2d => {
                if data.len() < 16 {
                    return None;
                }
                let x = f64::from_le_bytes([
                    data[0], data[1], data[2], data[3], data[4], data[5], data[6], data[7],
                ]);
                let y = f64::from_le_bytes([
                    data[8], data[9], data[10], data[11], data[12], data[13], data[14], data[15],
                ]);
                Some(usd_vt::Value::from_no_hash(usd_gf::Vec2d::new(x, y)))
            }
            TypeEnum::Vec2f => {
                if data.len() < 8 {
                    return None;
                }
                let x = f32::from_le_bytes([data[0], data[1], data[2], data[3]]);
                let y = f32::from_le_bytes([data[4], data[5], data[6], data[7]]);
                Some(usd_vt::Value::from_no_hash(usd_gf::Vec2f::new(x, y)))
            }
            TypeEnum::Vec2i => {
                if data.len() < 8 {
                    return None;
                }
                let x = i32::from_le_bytes([data[0], data[1], data[2], data[3]]);
                let y = i32::from_le_bytes([data[4], data[5], data[6], data[7]]);
                Some(usd_vt::Value::new(usd_gf::Vec2i::new(x, y)))
            }
            TypeEnum::Vec2h => {
                if data.len() < 4 {
                    return None;
                }
                let x = half::f16::from_le_bytes([data[0], data[1]]);
                let y = half::f16::from_le_bytes([data[2], data[3]]);
                Some(usd_vt::Value::from_no_hash(usd_gf::Vec2f::new(
                    x.to_f32(),
                    y.to_f32(),
                )))
            }
            _ => None,
        }
    }

    /// Unpacks Vec3 from file data.
    fn unpack_vec3(&self, type_enum: TypeEnum, data: &[u8]) -> Option<usd_vt::Value> {
        match type_enum {
            TypeEnum::Vec3d => {
                if data.len() < 24 {
                    return None;
                }
                let x = f64::from_le_bytes([
                    data[0], data[1], data[2], data[3], data[4], data[5], data[6], data[7],
                ]);
                let y = f64::from_le_bytes([
                    data[8], data[9], data[10], data[11], data[12], data[13], data[14], data[15],
                ]);
                let z = f64::from_le_bytes([
                    data[16], data[17], data[18], data[19], data[20], data[21], data[22], data[23],
                ]);
                Some(usd_vt::Value::from_no_hash(usd_gf::Vec3d::new(x, y, z)))
            }
            TypeEnum::Vec3f => {
                if data.len() < 12 {
                    return None;
                }
                let x = f32::from_le_bytes([data[0], data[1], data[2], data[3]]);
                let y = f32::from_le_bytes([data[4], data[5], data[6], data[7]]);
                let z = f32::from_le_bytes([data[8], data[9], data[10], data[11]]);
                Some(usd_vt::Value::from_no_hash(usd_gf::Vec3f::new(x, y, z)))
            }
            TypeEnum::Vec3i => {
                if data.len() < 12 {
                    return None;
                }
                let x = i32::from_le_bytes([data[0], data[1], data[2], data[3]]);
                let y = i32::from_le_bytes([data[4], data[5], data[6], data[7]]);
                let z = i32::from_le_bytes([data[8], data[9], data[10], data[11]]);
                Some(usd_vt::Value::new(usd_gf::Vec3i::new(x, y, z)))
            }
            TypeEnum::Vec3h => {
                if data.len() < 6 {
                    return None;
                }
                let x = half::f16::from_le_bytes([data[0], data[1]]);
                let y = half::f16::from_le_bytes([data[2], data[3]]);
                let z = half::f16::from_le_bytes([data[4], data[5]]);
                Some(usd_vt::Value::from_no_hash(usd_gf::Vec3f::new(
                    x.to_f32(),
                    y.to_f32(),
                    z.to_f32(),
                )))
            }
            _ => None,
        }
    }

    /// Unpacks Vec4 from file data.
    fn unpack_vec4(&self, type_enum: TypeEnum, data: &[u8]) -> Option<usd_vt::Value> {
        match type_enum {
            TypeEnum::Vec4d => {
                if data.len() < 32 {
                    return None;
                }
                let x = f64::from_le_bytes([
                    data[0], data[1], data[2], data[3], data[4], data[5], data[6], data[7],
                ]);
                let y = f64::from_le_bytes([
                    data[8], data[9], data[10], data[11], data[12], data[13], data[14], data[15],
                ]);
                let z = f64::from_le_bytes([
                    data[16], data[17], data[18], data[19], data[20], data[21], data[22], data[23],
                ]);
                let w = f64::from_le_bytes([
                    data[24], data[25], data[26], data[27], data[28], data[29], data[30], data[31],
                ]);
                Some(usd_vt::Value::from_no_hash(usd_gf::Vec4d::new(x, y, z, w)))
            }
            TypeEnum::Vec4f => {
                if data.len() < 16 {
                    return None;
                }
                let x = f32::from_le_bytes([data[0], data[1], data[2], data[3]]);
                let y = f32::from_le_bytes([data[4], data[5], data[6], data[7]]);
                let z = f32::from_le_bytes([data[8], data[9], data[10], data[11]]);
                let w = f32::from_le_bytes([data[12], data[13], data[14], data[15]]);
                Some(usd_vt::Value::from_no_hash(usd_gf::Vec4f::new(x, y, z, w)))
            }
            TypeEnum::Vec4i => {
                if data.len() < 16 {
                    return None;
                }
                let x = i32::from_le_bytes([data[0], data[1], data[2], data[3]]);
                let y = i32::from_le_bytes([data[4], data[5], data[6], data[7]]);
                let z = i32::from_le_bytes([data[8], data[9], data[10], data[11]]);
                let w = i32::from_le_bytes([data[12], data[13], data[14], data[15]]);
                Some(usd_vt::Value::new(usd_gf::Vec4i::new(x, y, z, w)))
            }
            TypeEnum::Vec4h => {
                if data.len() < 8 {
                    return None;
                }
                let x = half::f16::from_le_bytes([data[0], data[1]]);
                let y = half::f16::from_le_bytes([data[2], data[3]]);
                let z = half::f16::from_le_bytes([data[4], data[5]]);
                let w = half::f16::from_le_bytes([data[6], data[7]]);
                Some(usd_vt::Value::from_no_hash(usd_gf::Vec4f::new(
                    x.to_f32(),
                    y.to_f32(),
                    z.to_f32(),
                    w.to_f32(),
                )))
            }
            _ => None,
        }
    }

    /// Unpacks quaternion from file data.
    fn unpack_quat(&self, type_enum: TypeEnum, data: &[u8]) -> Option<usd_vt::Value> {
        match type_enum {
            TypeEnum::Quatd => {
                if data.len() < 32 {
                    return None;
                }
                let i = f64::from_le_bytes([
                    data[0], data[1], data[2], data[3], data[4], data[5], data[6], data[7],
                ]);
                let j = f64::from_le_bytes([
                    data[8], data[9], data[10], data[11], data[12], data[13], data[14], data[15],
                ]);
                let k = f64::from_le_bytes([
                    data[16], data[17], data[18], data[19], data[20], data[21], data[22], data[23],
                ]);
                let r = f64::from_le_bytes([
                    data[24], data[25], data[26], data[27], data[28], data[29], data[30], data[31],
                ]);
                Some(usd_vt::Value::from_no_hash(usd_gf::Quatd::new(
                    r,
                    usd_gf::Vec3d::new(i, j, k),
                )))
            }
            TypeEnum::Quatf => {
                if data.len() < 16 {
                    return None;
                }
                let i = f32::from_le_bytes([data[0], data[1], data[2], data[3]]);
                let j = f32::from_le_bytes([data[4], data[5], data[6], data[7]]);
                let k = f32::from_le_bytes([data[8], data[9], data[10], data[11]]);
                let r = f32::from_le_bytes([data[12], data[13], data[14], data[15]]);
                Some(usd_vt::Value::from_no_hash(usd_gf::Quatf::new(
                    r,
                    usd_gf::Vec3f::new(i, j, k),
                )))
            }
            TypeEnum::Quath => {
                if data.len() < 8 {
                    return None;
                }
                let i = half::f16::from_le_bytes([data[0], data[1]]).to_f32();
                let j = half::f16::from_le_bytes([data[2], data[3]]).to_f32();
                let k = half::f16::from_le_bytes([data[4], data[5]]).to_f32();
                let r = half::f16::from_le_bytes([data[6], data[7]]).to_f32();
                Some(usd_vt::Value::from_no_hash(usd_gf::Quatf::new(
                    r,
                    usd_gf::Vec3f::new(i, j, k),
                )))
            }
            _ => None,
        }
    }

    /// Unpacks Matrix2d from file data.
    fn unpack_matrix2d(&self, data: &[u8]) -> Option<usd_vt::Value> {
        if data.len() < 32 {
            return None;
        }
        let mut values = [[0.0f64; 2]; 2];
        for i in 0..2 {
            for j in 0..2 {
                let off = (i * 2 + j) * 8;
                values[i][j] = f64::from_le_bytes([
                    data[off],
                    data[off + 1],
                    data[off + 2],
                    data[off + 3],
                    data[off + 4],
                    data[off + 5],
                    data[off + 6],
                    data[off + 7],
                ]);
            }
        }
        Some(usd_vt::Value::from_no_hash(usd_gf::Matrix2d::from_array(
            values,
        )))
    }

    /// Unpacks Matrix3d from file data.
    fn unpack_matrix3d(&self, data: &[u8]) -> Option<usd_vt::Value> {
        if data.len() < 72 {
            return None;
        }
        let mut values = [[0.0f64; 3]; 3];
        for i in 0..3 {
            for j in 0..3 {
                let off = (i * 3 + j) * 8;
                values[i][j] = f64::from_le_bytes([
                    data[off],
                    data[off + 1],
                    data[off + 2],
                    data[off + 3],
                    data[off + 4],
                    data[off + 5],
                    data[off + 6],
                    data[off + 7],
                ]);
            }
        }
        Some(usd_vt::Value::from_no_hash(usd_gf::Matrix3d::from_array(
            values,
        )))
    }

    /// Unpacks Matrix4d from file data.
    fn unpack_matrix4d(&self, data: &[u8]) -> Option<usd_vt::Value> {
        if data.len() < 128 {
            return None;
        }
        let mut values = [[0.0f64; 4]; 4];
        for i in 0..4 {
            for j in 0..4 {
                let off = (i * 4 + j) * 8;
                values[i][j] = f64::from_le_bytes([
                    data[off],
                    data[off + 1],
                    data[off + 2],
                    data[off + 3],
                    data[off + 4],
                    data[off + 5],
                    data[off + 6],
                    data[off + 7],
                ]);
            }
        }
        Some(usd_vt::Value::from_no_hash(usd_gf::Matrix4d::from_array(
            values,
        )))
    }

    /// Unpacks array value from file data.
    /// Minimum array size for compression (matches C++ MinCompressedArraySize)
    const MIN_COMPRESSED_ARRAY_SIZE: usize = 16;

    /// Unpacks an array value from file data.
    /// Handles both compressed (version >= 0.5.0) and uncompressed arrays.
    fn unpack_array_value(
        &self,
        rep: &ValueRep,
        offset: usize,
        crate_file: &CrateFile,
        file_data: &[u8],
    ) -> Option<usd_vt::Value> {
        let data = &file_data[offset..];
        let type_enum = rep.get_type();
        let ver = crate_file.version;
        let is_compressed = rep.is_compressed();

        // Check version thresholds for compression support
        let supports_int_compression = ver >= (0, 5, 0);
        let supports_float_compression = ver >= (0, 6, 0);
        let uses_64bit_size = ver >= (0, 7, 0);

        // Determine if this array uses compressed format based on version and flag
        let use_compressed_format = match type_enum {
            TypeEnum::Int | TypeEnum::UInt | TypeEnum::Int64 | TypeEnum::UInt64 => {
                supports_int_compression && is_compressed
            }
            TypeEnum::Half | TypeEnum::Float | TypeEnum::Double => {
                supports_float_compression && is_compressed
            }
            _ => false,
        };
        // Token arrays: stored as u32 TokenIndex into token table
        if type_enum == TypeEnum::Token {
            return self.unpack_token_array(data, crate_file, uses_64bit_size);
        }

        // String and AssetPath arrays: stored as u32 StringIndex into string table.
        // C++ crateFile.cpp:1167-1190: Read<string>() dispatches to
        //   crate->GetString(Read<StringIndex>())
        // Read<SdfAssetPath>() dispatches to T(Read<string>())
        // _ReadUncompressedArray calls Read<T>() for each element.
        if type_enum == TypeEnum::String {
            return self.unpack_string_array(data, crate_file, uses_64bit_size);
        }
        if type_enum == TypeEnum::AssetPath {
            return self.unpack_asset_path_array(data, crate_file, uses_64bit_size);
        }

        if use_compressed_format {
            // Compressed format: size + compressed data
            self.unpack_compressed_array(type_enum, data, uses_64bit_size)
        } else {
            // Uncompressed format: num_elements + raw data
            self.unpack_uncompressed_array(type_enum, data, uses_64bit_size)
        }
    }

    /// Unpacks a Token array (array of token indices → resolved token strings).
    /// C++ reference: _ReadTokenArray reads size + compressed/raw u32 indices.
    fn unpack_token_array(
        &self,
        data: &[u8],
        crate_file: &CrateFile,
        uses_64bit_size: bool,
    ) -> Option<usd_vt::Value> {
        let (num_elements, header_size) = if uses_64bit_size {
            if data.len() < 8 {
                return None;
            }
            let n = u64::from_le_bytes([
                data[0], data[1], data[2], data[3], data[4], data[5], data[6], data[7],
            ]) as usize;
            (n, 8)
        } else {
            if data.len() < 4 {
                return None;
            }
            let n = u32::from_le_bytes([data[0], data[1], data[2], data[3]]) as usize;
            (n, 4)
        };
        if num_elements == 0 {
            return Some(usd_vt::Value::new(usd_vt::Array::<Token>::new()));
        }
        // Sanity check: token arrays should not exceed available data
        let idx_data = &data[header_size..];
        // Each token index is u32
        let needed = match num_elements.checked_mul(4) {
            Some(n) => n,
            None => return None, // overflow = corrupt data
        };
        if idx_data.len() < needed {
            return None;
        }
        let mut tokens: Vec<Token> = Vec::with_capacity(num_elements);
        for i in 0..num_elements {
            let off = i * 4;
            let idx = u32::from_le_bytes([
                idx_data[off],
                idx_data[off + 1],
                idx_data[off + 2],
                idx_data[off + 3],
            ]);
            let token_str = crate_file
                .get_token(TokenIndex(Index { value: idx }))
                .map(|t| t.as_str().to_string())
                .unwrap_or_default();
            tokens.push(Token::new(&token_str));
        }
        Some(usd_vt::Value::new(usd_vt::Array::from(tokens)))
    }

    /// Unpacks a String array (array of StringIndex → resolved strings).
    ///
    /// C++ crateFile.cpp:1167-1168: `Read<string>()` dispatches to
    /// `crate->GetString(Read<StringIndex>())`. Each element is a u32 index
    /// into the string table, resolved to the actual string value.
    fn unpack_string_array(
        &self,
        data: &[u8],
        crate_file: &CrateFile,
        uses_64bit_size: bool,
    ) -> Option<usd_vt::Value> {
        let (num_elements, header_size) = if uses_64bit_size {
            if data.len() < 8 {
                return None;
            }
            let n = u64::from_le_bytes([
                data[0], data[1], data[2], data[3], data[4], data[5], data[6], data[7],
            ]) as usize;
            (n, 8)
        } else {
            if data.len() < 4 {
                return None;
            }
            let n = u32::from_le_bytes([data[0], data[1], data[2], data[3]]) as usize;
            (n, 4)
        };
        if num_elements == 0 {
            return Some(usd_vt::Value::new(usd_vt::Array::<String>::new()));
        }
        let idx_data = &data[header_size..];
        let needed = num_elements.checked_mul(4)?;
        if idx_data.len() < needed {
            return None;
        }

        let mut strings: Vec<String> = Vec::with_capacity(num_elements);
        for i in 0..num_elements {
            let off = i * 4;
            let idx = u32::from_le_bytes([
                idx_data[off],
                idx_data[off + 1],
                idx_data[off + 2],
                idx_data[off + 3],
            ]);
            let s = crate_file
                .get_string(StringIndex(Index { value: idx }))
                .unwrap_or_default()
                .to_string();
            strings.push(s);
        }
        Some(usd_vt::Value::new(usd_vt::Array::from(strings)))
    }

    /// Unpacks an AssetPath array (array of StringIndex → resolved asset paths).
    ///
    /// C++ crateFile.cpp:1188-1190: `Read<SdfAssetPath>()` dispatches to
    /// `T(Read<string>())` which reads a StringIndex, resolves it to a string,
    /// then wraps in SdfAssetPath. Same binary layout as string arrays.
    fn unpack_asset_path_array(
        &self,
        data: &[u8],
        crate_file: &CrateFile,
        uses_64bit_size: bool,
    ) -> Option<usd_vt::Value> {
        let (num_elements, header_size) = if uses_64bit_size {
            if data.len() < 8 {
                return None;
            }
            let n = u64::from_le_bytes([
                data[0], data[1], data[2], data[3], data[4], data[5], data[6], data[7],
            ]) as usize;
            (n, 8)
        } else {
            if data.len() < 4 {
                return None;
            }
            let n = u32::from_le_bytes([data[0], data[1], data[2], data[3]]) as usize;
            (n, 4)
        };
        if num_elements == 0 {
            return Some(usd_vt::Value::new(usd_vt::Array::<
                super::asset_path::AssetPath,
            >::new()));
        }
        let idx_data = &data[header_size..];
        let needed = num_elements.checked_mul(4)?;
        if idx_data.len() < needed {
            return None;
        }

        let mut paths: Vec<super::asset_path::AssetPath> = Vec::with_capacity(num_elements);
        for i in 0..num_elements {
            let off = i * 4;
            let idx = u32::from_le_bytes([
                idx_data[off],
                idx_data[off + 1],
                idx_data[off + 2],
                idx_data[off + 3],
            ]);
            let s = crate_file
                .get_string(StringIndex(Index { value: idx }))
                .unwrap_or_default();
            paths.push(super::asset_path::AssetPath::new(s));
        }
        Some(usd_vt::Value::new(usd_vt::Array::from(paths)))
    }

    /// Unpacks an uncompressed array.
    /// C++ reference: _ReadArray reads size (u32 or u64) + raw data.
    fn unpack_uncompressed_array(
        &self,
        type_enum: TypeEnum,
        data: &[u8],
        uses_64bit_size: bool,
    ) -> Option<usd_vt::Value> {
        let (num_elements, header_size) = if uses_64bit_size {
            if data.len() < 8 {
                return None;
            }
            let n = u64::from_le_bytes([
                data[0], data[1], data[2], data[3], data[4], data[5], data[6], data[7],
            ]) as usize;
            (n, 8)
        } else {
            if data.len() < 4 {
                return None;
            }
            let n = u32::from_le_bytes([data[0], data[1], data[2], data[3]]) as usize;
            (n, 4)
        };
        let array_data = &data[header_size..];

        match type_enum {
            TypeEnum::Int => self.read_raw_i32_array(array_data, num_elements),
            TypeEnum::UInt => self.read_raw_u32_array(array_data, num_elements),
            TypeEnum::Int64 => self.read_raw_i64_array(array_data, num_elements),
            TypeEnum::UInt64 => self.read_raw_u64_array(array_data, num_elements),
            TypeEnum::Half => self.read_raw_half_array(array_data, num_elements),
            TypeEnum::Float => self.read_raw_f32_array(array_data, num_elements),
            TypeEnum::Double => self.read_raw_f64_array(array_data, num_elements),
            TypeEnum::Vec2f => self.read_raw_vec2f_array(array_data, num_elements),
            TypeEnum::Vec3f => self.read_raw_vec3f_array(array_data, num_elements),
            TypeEnum::Vec4f => self.read_raw_vec4f_array(array_data, num_elements),
            TypeEnum::Vec2d => self.read_raw_vec2d_array(array_data, num_elements),
            TypeEnum::Vec3d => self.read_raw_vec3d_array(array_data, num_elements),
            TypeEnum::Vec4d => self.read_raw_vec4d_array(array_data, num_elements),
            TypeEnum::Vec2i => self.read_raw_vec2i_array(array_data, num_elements),
            TypeEnum::Vec3i => self.read_raw_vec3i_array(array_data, num_elements),
            TypeEnum::Vec4i => self.read_raw_vec4i_array(array_data, num_elements),
            TypeEnum::Vec2h => self.read_raw_vec2h_array(array_data, num_elements),
            TypeEnum::Vec3h => self.read_raw_vec3h_array(array_data, num_elements),
            TypeEnum::Vec4h => self.read_raw_vec4h_array(array_data, num_elements),
            TypeEnum::Quatf => self.read_raw_quatf_array(array_data, num_elements),
            TypeEnum::Quatd => self.read_raw_quatd_array(array_data, num_elements),
            TypeEnum::Quath => self.read_raw_quath_array(array_data, num_elements),
            TypeEnum::Matrix2d => self.read_raw_matrix2d_array(array_data, num_elements),
            TypeEnum::Matrix3d => self.read_raw_matrix3d_array(array_data, num_elements),
            TypeEnum::Matrix4d => self.read_raw_matrix4d_array(array_data, num_elements),
            TypeEnum::Bool => self.read_raw_bool_array(array_data, num_elements),
            _ => None,
        }
    }

    /// Unpacks a compressed array (version >= 0.5.0 for ints, >= 0.6.0 for floats).
    fn unpack_compressed_array(
        &self,
        type_enum: TypeEnum,
        data: &[u8],
        uses_64bit_size: bool,
    ) -> Option<usd_vt::Value> {
        // Read array size
        let (num_elements, mut offset) = if uses_64bit_size {
            if data.len() < 8 {
                return None;
            }
            let n = u64::from_le_bytes([
                data[0], data[1], data[2], data[3], data[4], data[5], data[6], data[7],
            ]) as usize;
            (n, 8)
        } else {
            if data.len() < 4 {
                return None;
            }
            let n = u32::from_le_bytes([data[0], data[1], data[2], data[3]]) as usize;
            (n, 4)
        };

        if num_elements == 0 {
            // Return empty array of appropriate type
            return self.make_empty_array(type_enum);
        }

        // If below MinCompressedArraySize, data is stored uncompressed
        if num_elements < Self::MIN_COMPRESSED_ARRAY_SIZE {
            let array_data = &data[offset..];
            return match type_enum {
                TypeEnum::Int => self.read_raw_i32_array(array_data, num_elements),
                TypeEnum::UInt => self.read_raw_u32_array(array_data, num_elements),
                TypeEnum::Int64 => self.read_raw_i64_array(array_data, num_elements),
                TypeEnum::UInt64 => self.read_raw_u64_array(array_data, num_elements),
                TypeEnum::Half => self.read_raw_half_array(array_data, num_elements),
                TypeEnum::Float => self.read_raw_f32_array(array_data, num_elements),
                TypeEnum::Double => self.read_raw_f64_array(array_data, num_elements),
                _ => None,
            };
        }

        // Compressed data
        match type_enum {
            TypeEnum::Int => self.read_compressed_i32_array(&data[offset..], num_elements),
            TypeEnum::UInt => self.read_compressed_u32_array(&data[offset..], num_elements),
            TypeEnum::Int64 => self.read_compressed_i64_array(&data[offset..], num_elements),
            TypeEnum::UInt64 => self.read_compressed_u64_array(&data[offset..], num_elements),
            TypeEnum::Half | TypeEnum::Float | TypeEnum::Double => {
                // Float compression uses a code byte
                if data.len() <= offset {
                    return None;
                }
                let code = data[offset] as char;
                offset += 1;
                self.read_compressed_float_array(type_enum, &data[offset..], num_elements, code)
            }
            _ => None,
        }
    }

    /// Creates an empty array of the specified type.
    /// C++ returns VtArray<T>() for all registered array types.
    fn make_empty_array(&self, type_enum: TypeEnum) -> Option<usd_vt::Value> {
        match type_enum {
            TypeEnum::Bool => Some(usd_vt::Value::from_no_hash(usd_vt::Array::<bool>::new())),
            TypeEnum::Int => Some(usd_vt::Value::from_no_hash(usd_vt::Array::<i32>::new())),
            TypeEnum::UInt => Some(usd_vt::Value::from_no_hash(usd_vt::Array::<u32>::new())),
            TypeEnum::Int64 => Some(usd_vt::Value::from_no_hash(usd_vt::Array::<i64>::new())),
            TypeEnum::UInt64 => Some(usd_vt::Value::from_no_hash(usd_vt::Array::<u64>::new())),
            TypeEnum::Half | TypeEnum::Float => {
                Some(usd_vt::Value::from_no_hash(usd_vt::Array::<f32>::new()))
            }
            TypeEnum::Double => Some(usd_vt::Value::from_no_hash(usd_vt::Array::<f64>::new())),
            TypeEnum::Token => Some(usd_vt::Value::from_no_hash(usd_vt::Array::<Token>::new())),
            TypeEnum::String => Some(usd_vt::Value::from_no_hash(usd_vt::Array::<String>::new())),
            TypeEnum::AssetPath => Some(usd_vt::Value::from_no_hash(usd_vt::Array::<
                crate::AssetPath,
            >::new())),
            TypeEnum::Vec2f => Some(usd_vt::Value::from_no_hash(
                usd_vt::Array::<usd_gf::Vec2f>::new(),
            )),
            TypeEnum::Vec2d => Some(usd_vt::Value::from_no_hash(
                usd_vt::Array::<usd_gf::Vec2d>::new(),
            )),
            TypeEnum::Vec2i => Some(usd_vt::Value::from_no_hash(
                usd_vt::Array::<usd_gf::Vec2i>::new(),
            )),
            TypeEnum::Vec2h => Some(usd_vt::Value::from_no_hash(
                usd_vt::Array::<usd_gf::Vec2h>::new(),
            )),
            TypeEnum::Vec3f => Some(usd_vt::Value::from_no_hash(
                usd_vt::Array::<usd_gf::Vec3f>::new(),
            )),
            TypeEnum::Vec3d => Some(usd_vt::Value::from_no_hash(
                usd_vt::Array::<usd_gf::Vec3d>::new(),
            )),
            TypeEnum::Vec3i => Some(usd_vt::Value::from_no_hash(
                usd_vt::Array::<usd_gf::Vec3i>::new(),
            )),
            TypeEnum::Vec3h => Some(usd_vt::Value::from_no_hash(
                usd_vt::Array::<usd_gf::Vec3h>::new(),
            )),
            TypeEnum::Vec4f => Some(usd_vt::Value::from_no_hash(
                usd_vt::Array::<usd_gf::Vec4f>::new(),
            )),
            TypeEnum::Vec4d => Some(usd_vt::Value::from_no_hash(
                usd_vt::Array::<usd_gf::Vec4d>::new(),
            )),
            TypeEnum::Vec4i => Some(usd_vt::Value::from_no_hash(
                usd_vt::Array::<usd_gf::Vec4i>::new(),
            )),
            TypeEnum::Vec4h => Some(usd_vt::Value::from_no_hash(
                usd_vt::Array::<usd_gf::Vec4h>::new(),
            )),
            TypeEnum::Quatf => Some(usd_vt::Value::from_no_hash(
                usd_vt::Array::<usd_gf::Quatf>::new(),
            )),
            TypeEnum::Quatd => Some(usd_vt::Value::from_no_hash(
                usd_vt::Array::<usd_gf::Quatd>::new(),
            )),
            TypeEnum::Quath => Some(usd_vt::Value::from_no_hash(
                usd_vt::Array::<usd_gf::Quath>::new(),
            )),
            TypeEnum::Matrix2d => Some(usd_vt::Value::from_no_hash(usd_vt::Array::<
                usd_gf::Matrix2d,
            >::new())),
            TypeEnum::Matrix3d => Some(usd_vt::Value::from_no_hash(usd_vt::Array::<
                usd_gf::Matrix3d,
            >::new())),
            TypeEnum::Matrix4d => Some(usd_vt::Value::from_no_hash(usd_vt::Array::<
                usd_gf::Matrix4d,
            >::new())),
            _ => None,
        }
    }

    // --- Raw (uncompressed) array readers ---

    fn read_raw_i32_array(&self, data: &[u8], n: usize) -> Option<usd_vt::Value> {
        if data.len() < n * 4 {
            return None;
        }
        let mut arr = Vec::with_capacity(n);
        for i in 0..n {
            let off = i * 4;
            arr.push(i32::from_le_bytes([
                data[off],
                data[off + 1],
                data[off + 2],
                data[off + 3],
            ]));
        }
        Some(usd_vt::Value::from_no_hash(usd_vt::Array::from(arr)))
    }

    fn read_raw_u32_array(&self, data: &[u8], n: usize) -> Option<usd_vt::Value> {
        if data.len() < n * 4 {
            return None;
        }
        let mut arr = Vec::with_capacity(n);
        for i in 0..n {
            let off = i * 4;
            arr.push(u32::from_le_bytes([
                data[off],
                data[off + 1],
                data[off + 2],
                data[off + 3],
            ]));
        }
        Some(usd_vt::Value::from_no_hash(usd_vt::Array::from(arr)))
    }

    fn read_raw_i64_array(&self, data: &[u8], n: usize) -> Option<usd_vt::Value> {
        if data.len() < n * 8 {
            return None;
        }
        let mut arr = Vec::with_capacity(n);
        for i in 0..n {
            let off = i * 8;
            arr.push(i64::from_le_bytes([
                data[off],
                data[off + 1],
                data[off + 2],
                data[off + 3],
                data[off + 4],
                data[off + 5],
                data[off + 6],
                data[off + 7],
            ]));
        }
        Some(usd_vt::Value::from_no_hash(usd_vt::Array::from(arr)))
    }

    fn read_raw_u64_array(&self, data: &[u8], n: usize) -> Option<usd_vt::Value> {
        if data.len() < n * 8 {
            return None;
        }
        let mut arr = Vec::with_capacity(n);
        for i in 0..n {
            let off = i * 8;
            arr.push(u64::from_le_bytes([
                data[off],
                data[off + 1],
                data[off + 2],
                data[off + 3],
                data[off + 4],
                data[off + 5],
                data[off + 6],
                data[off + 7],
            ]));
        }
        Some(usd_vt::Value::from_no_hash(usd_vt::Array::from(arr)))
    }

    fn read_raw_half_array(&self, data: &[u8], n: usize) -> Option<usd_vt::Value> {
        if data.len() < n * 2 {
            return None;
        }
        let mut arr = Vec::with_capacity(n);
        for i in 0..n {
            let off = i * 2;
            let bits = u16::from_le_bytes([data[off], data[off + 1]]);
            // Convert half to f32 (simplified - full conversion would need proper half-float math)
            arr.push(half_to_f32(bits));
        }
        Some(usd_vt::Value::from_no_hash(usd_vt::Array::from(arr)))
    }

    fn read_raw_f32_array(&self, data: &[u8], n: usize) -> Option<usd_vt::Value> {
        if data.len() < n * 4 {
            return None;
        }
        let mut arr = Vec::with_capacity(n);
        for i in 0..n {
            let off = i * 4;
            let bits = u32::from_le_bytes([data[off], data[off + 1], data[off + 2], data[off + 3]]);
            arr.push(f32::from_bits(bits));
        }
        Some(usd_vt::Value::from_no_hash(usd_vt::Array::from(arr)))
    }

    fn read_raw_f64_array(&self, data: &[u8], n: usize) -> Option<usd_vt::Value> {
        if data.len() < n * 8 {
            return None;
        }
        let mut arr = Vec::with_capacity(n);
        for i in 0..n {
            let off = i * 8;
            let bits = u64::from_le_bytes([
                data[off],
                data[off + 1],
                data[off + 2],
                data[off + 3],
                data[off + 4],
                data[off + 5],
                data[off + 6],
                data[off + 7],
            ]);
            arr.push(f64::from_bits(bits));
        }
        Some(usd_vt::Value::from_no_hash(usd_vt::Array::from(arr)))
    }

    fn read_raw_vec2f_array(&self, data: &[u8], n: usize) -> Option<usd_vt::Value> {
        if data.len() < n * 8 {
            return None;
        }
        let mut arr = Vec::with_capacity(n);
        for i in 0..n {
            let off = i * 8;
            let x = f32::from_le_bytes([data[off], data[off + 1], data[off + 2], data[off + 3]]);
            let y =
                f32::from_le_bytes([data[off + 4], data[off + 5], data[off + 6], data[off + 7]]);
            arr.push(usd_gf::Vec2f::new(x, y));
        }
        Some(usd_vt::Value::from_no_hash(usd_vt::Array::from(arr)))
    }

    fn read_raw_vec3f_array(&self, data: &[u8], n: usize) -> Option<usd_vt::Value> {
        if data.len() < n * 12 {
            return None;
        }
        let mut arr = Vec::with_capacity(n);
        for i in 0..n {
            let off = i * 12;
            let x = f32::from_le_bytes([data[off], data[off + 1], data[off + 2], data[off + 3]]);
            let y =
                f32::from_le_bytes([data[off + 4], data[off + 5], data[off + 6], data[off + 7]]);
            let z =
                f32::from_le_bytes([data[off + 8], data[off + 9], data[off + 10], data[off + 11]]);
            arr.push(usd_gf::Vec3f::new(x, y, z));
        }
        Some(usd_vt::Value::from_no_hash(usd_vt::Array::from(arr)))
    }

    fn read_raw_vec4f_array(&self, data: &[u8], n: usize) -> Option<usd_vt::Value> {
        if data.len() < n * 16 {
            return None;
        }
        let mut arr = Vec::with_capacity(n);
        for i in 0..n {
            let off = i * 16;
            let x = f32::from_le_bytes([data[off], data[off + 1], data[off + 2], data[off + 3]]);
            let y =
                f32::from_le_bytes([data[off + 4], data[off + 5], data[off + 6], data[off + 7]]);
            let z =
                f32::from_le_bytes([data[off + 8], data[off + 9], data[off + 10], data[off + 11]]);
            let w = f32::from_le_bytes([
                data[off + 12],
                data[off + 13],
                data[off + 14],
                data[off + 15],
            ]);
            arr.push(usd_gf::Vec4f::new(x, y, z, w));
        }
        Some(usd_vt::Value::from_no_hash(usd_vt::Array::from(arr)))
    }

    fn read_raw_vec2d_array(&self, data: &[u8], n: usize) -> Option<usd_vt::Value> {
        if data.len() < n * 16 {
            return None;
        }
        let mut arr = Vec::with_capacity(n);
        for i in 0..n {
            let off = i * 16;
            let x = f64::from_bits(u64::from_le_bytes([
                data[off],
                data[off + 1],
                data[off + 2],
                data[off + 3],
                data[off + 4],
                data[off + 5],
                data[off + 6],
                data[off + 7],
            ]));
            let y = f64::from_bits(u64::from_le_bytes([
                data[off + 8],
                data[off + 9],
                data[off + 10],
                data[off + 11],
                data[off + 12],
                data[off + 13],
                data[off + 14],
                data[off + 15],
            ]));
            arr.push(usd_gf::Vec2d::new(x, y));
        }
        Some(usd_vt::Value::from_no_hash(usd_vt::Array::from(arr)))
    }

    fn read_raw_vec3d_array(&self, data: &[u8], n: usize) -> Option<usd_vt::Value> {
        if data.len() < n * 24 {
            return None;
        }
        let mut arr = Vec::with_capacity(n);
        for i in 0..n {
            let off = i * 24;
            let x = f64::from_bits(u64::from_le_bytes([
                data[off],
                data[off + 1],
                data[off + 2],
                data[off + 3],
                data[off + 4],
                data[off + 5],
                data[off + 6],
                data[off + 7],
            ]));
            let y = f64::from_bits(u64::from_le_bytes([
                data[off + 8],
                data[off + 9],
                data[off + 10],
                data[off + 11],
                data[off + 12],
                data[off + 13],
                data[off + 14],
                data[off + 15],
            ]));
            let z = f64::from_bits(u64::from_le_bytes([
                data[off + 16],
                data[off + 17],
                data[off + 18],
                data[off + 19],
                data[off + 20],
                data[off + 21],
                data[off + 22],
                data[off + 23],
            ]));
            arr.push(usd_gf::Vec3d::new(x, y, z));
        }
        Some(usd_vt::Value::from_no_hash(usd_vt::Array::from(arr)))
    }

    fn read_raw_vec4d_array(&self, data: &[u8], n: usize) -> Option<usd_vt::Value> {
        if data.len() < n * 32 {
            return None;
        }
        let mut arr = Vec::with_capacity(n);
        for i in 0..n {
            let off = i * 32;
            let x = f64::from_bits(u64::from_le_bytes([
                data[off],
                data[off + 1],
                data[off + 2],
                data[off + 3],
                data[off + 4],
                data[off + 5],
                data[off + 6],
                data[off + 7],
            ]));
            let y = f64::from_bits(u64::from_le_bytes([
                data[off + 8],
                data[off + 9],
                data[off + 10],
                data[off + 11],
                data[off + 12],
                data[off + 13],
                data[off + 14],
                data[off + 15],
            ]));
            let z = f64::from_bits(u64::from_le_bytes([
                data[off + 16],
                data[off + 17],
                data[off + 18],
                data[off + 19],
                data[off + 20],
                data[off + 21],
                data[off + 22],
                data[off + 23],
            ]));
            let w = f64::from_bits(u64::from_le_bytes([
                data[off + 24],
                data[off + 25],
                data[off + 26],
                data[off + 27],
                data[off + 28],
                data[off + 29],
                data[off + 30],
                data[off + 31],
            ]));
            arr.push(usd_gf::Vec4d::new(x, y, z, w));
        }
        Some(usd_vt::Value::from_no_hash(usd_vt::Array::from(arr)))
    }

    // --- Additional raw array readers for int/half/quat/matrix types ---

    fn read_raw_vec2i_array(&self, data: &[u8], n: usize) -> Option<usd_vt::Value> {
        if data.len() < n * 8 {
            return None;
        }
        let mut arr = Vec::with_capacity(n);
        for i in 0..n {
            let off = i * 8;
            let x = i32::from_le_bytes([data[off], data[off + 1], data[off + 2], data[off + 3]]);
            let y =
                i32::from_le_bytes([data[off + 4], data[off + 5], data[off + 6], data[off + 7]]);
            arr.push(usd_gf::Vec2i::new(x, y));
        }
        Some(usd_vt::Value::from_no_hash(usd_vt::Array::from(arr)))
    }

    fn read_raw_vec3i_array(&self, data: &[u8], n: usize) -> Option<usd_vt::Value> {
        if data.len() < n * 12 {
            return None;
        }
        let mut arr = Vec::with_capacity(n);
        for i in 0..n {
            let off = i * 12;
            let x = i32::from_le_bytes([data[off], data[off + 1], data[off + 2], data[off + 3]]);
            let y =
                i32::from_le_bytes([data[off + 4], data[off + 5], data[off + 6], data[off + 7]]);
            let z =
                i32::from_le_bytes([data[off + 8], data[off + 9], data[off + 10], data[off + 11]]);
            arr.push(usd_gf::Vec3i::new(x, y, z));
        }
        Some(usd_vt::Value::from_no_hash(usd_vt::Array::from(arr)))
    }

    fn read_raw_vec4i_array(&self, data: &[u8], n: usize) -> Option<usd_vt::Value> {
        if data.len() < n * 16 {
            return None;
        }
        let mut arr = Vec::with_capacity(n);
        for i in 0..n {
            let off = i * 16;
            let x = i32::from_le_bytes([data[off], data[off + 1], data[off + 2], data[off + 3]]);
            let y =
                i32::from_le_bytes([data[off + 4], data[off + 5], data[off + 6], data[off + 7]]);
            let z =
                i32::from_le_bytes([data[off + 8], data[off + 9], data[off + 10], data[off + 11]]);
            let w = i32::from_le_bytes([
                data[off + 12],
                data[off + 13],
                data[off + 14],
                data[off + 15],
            ]);
            arr.push(usd_gf::Vec4i::new(x, y, z, w));
        }
        Some(usd_vt::Value::from_no_hash(usd_vt::Array::from(arr)))
    }

    fn read_raw_vec2h_array(&self, data: &[u8], n: usize) -> Option<usd_vt::Value> {
        if data.len() < n * 4 {
            return None;
        }
        let mut arr = Vec::with_capacity(n);
        for i in 0..n {
            let off = i * 4;
            let x = usd_gf::Half::from_bits(u16::from_le_bytes([data[off], data[off + 1]]));
            let y = usd_gf::Half::from_bits(u16::from_le_bytes([data[off + 2], data[off + 3]]));
            arr.push(usd_gf::Vec2h::new(x, y));
        }
        Some(usd_vt::Value::from_no_hash(usd_vt::Array::from(arr)))
    }

    fn read_raw_vec3h_array(&self, data: &[u8], n: usize) -> Option<usd_vt::Value> {
        if data.len() < n * 6 {
            return None;
        }
        let mut arr = Vec::with_capacity(n);
        for i in 0..n {
            let off = i * 6;
            let x = usd_gf::Half::from_bits(u16::from_le_bytes([data[off], data[off + 1]]));
            let y = usd_gf::Half::from_bits(u16::from_le_bytes([data[off + 2], data[off + 3]]));
            let z = usd_gf::Half::from_bits(u16::from_le_bytes([data[off + 4], data[off + 5]]));
            arr.push(usd_gf::Vec3h::new(x, y, z));
        }
        Some(usd_vt::Value::from_no_hash(usd_vt::Array::from(arr)))
    }

    fn read_raw_vec4h_array(&self, data: &[u8], n: usize) -> Option<usd_vt::Value> {
        if data.len() < n * 8 {
            return None;
        }
        let mut arr = Vec::with_capacity(n);
        for i in 0..n {
            let off = i * 8;
            let x = usd_gf::Half::from_bits(u16::from_le_bytes([data[off], data[off + 1]]));
            let y = usd_gf::Half::from_bits(u16::from_le_bytes([data[off + 2], data[off + 3]]));
            let z = usd_gf::Half::from_bits(u16::from_le_bytes([data[off + 4], data[off + 5]]));
            let w = usd_gf::Half::from_bits(u16::from_le_bytes([data[off + 6], data[off + 7]]));
            arr.push(usd_gf::Vec4h::new(x, y, z, w));
        }
        Some(usd_vt::Value::from_no_hash(usd_vt::Array::from(arr)))
    }

    /// Quatf: [imaginary.x, imaginary.y, imaginary.z, real] = 4 x f32 = 16 bytes
    fn read_raw_quatf_array(&self, data: &[u8], n: usize) -> Option<usd_vt::Value> {
        if data.len() < n * 16 {
            return None;
        }
        let mut arr = Vec::with_capacity(n);
        for i in 0..n {
            let off = i * 16;
            let ix = f32::from_le_bytes([data[off], data[off + 1], data[off + 2], data[off + 3]]);
            let iy =
                f32::from_le_bytes([data[off + 4], data[off + 5], data[off + 6], data[off + 7]]);
            let iz =
                f32::from_le_bytes([data[off + 8], data[off + 9], data[off + 10], data[off + 11]]);
            let r = f32::from_le_bytes([
                data[off + 12],
                data[off + 13],
                data[off + 14],
                data[off + 15],
            ]);
            arr.push(usd_gf::Quatf::new(r, usd_gf::Vec3f::new(ix, iy, iz)));
        }
        Some(usd_vt::Value::from_no_hash(usd_vt::Array::from(arr)))
    }

    /// Quatd: [imaginary.x, imaginary.y, imaginary.z, real] = 4 x f64 = 32 bytes
    fn read_raw_quatd_array(&self, data: &[u8], n: usize) -> Option<usd_vt::Value> {
        if data.len() < n * 32 {
            return None;
        }
        let mut arr = Vec::with_capacity(n);
        for i in 0..n {
            let off = i * 32;
            let read_f64 = |o: usize| -> f64 {
                f64::from_bits(u64::from_le_bytes([
                    data[o],
                    data[o + 1],
                    data[o + 2],
                    data[o + 3],
                    data[o + 4],
                    data[o + 5],
                    data[o + 6],
                    data[o + 7],
                ]))
            };
            let ix = read_f64(off);
            let iy = read_f64(off + 8);
            let iz = read_f64(off + 16);
            let r = read_f64(off + 24);
            arr.push(usd_gf::Quatd::new(r, usd_gf::Vec3d::new(ix, iy, iz)));
        }
        Some(usd_vt::Value::from_no_hash(usd_vt::Array::from(arr)))
    }

    /// Quath: [imaginary.x, imaginary.y, imaginary.z, real] = 4 x f16 = 8 bytes
    fn read_raw_quath_array(&self, data: &[u8], n: usize) -> Option<usd_vt::Value> {
        if data.len() < n * 8 {
            return None;
        }
        let mut arr = Vec::with_capacity(n);
        for i in 0..n {
            let off = i * 8;
            let ix = usd_gf::Half::from_bits(u16::from_le_bytes([data[off], data[off + 1]]));
            let iy = usd_gf::Half::from_bits(u16::from_le_bytes([data[off + 2], data[off + 3]]));
            let iz = usd_gf::Half::from_bits(u16::from_le_bytes([data[off + 4], data[off + 5]]));
            let r = usd_gf::Half::from_bits(u16::from_le_bytes([data[off + 6], data[off + 7]]));
            arr.push(usd_gf::Quath::new(r, usd_gf::Vec3h::new(ix, iy, iz)));
        }
        Some(usd_vt::Value::from_no_hash(usd_vt::Array::from(arr)))
    }

    /// Matrix2d: 4 x f64 = 32 bytes per element
    fn read_raw_matrix2d_array(&self, data: &[u8], n: usize) -> Option<usd_vt::Value> {
        let elem_size = 4 * 8; // 2x2 f64
        if data.len() < n * elem_size {
            return None;
        }
        let mut arr = Vec::with_capacity(n);
        for i in 0..n {
            let base = i * elem_size;
            let mut values = [[0.0f64; 2]; 2];
            for r in 0..2 {
                for c in 0..2 {
                    let off = base + (r * 2 + c) * 8;
                    values[r][c] = f64::from_bits(u64::from_le_bytes([
                        data[off],
                        data[off + 1],
                        data[off + 2],
                        data[off + 3],
                        data[off + 4],
                        data[off + 5],
                        data[off + 6],
                        data[off + 7],
                    ]));
                }
            }
            arr.push(usd_gf::Matrix2d::from_array(values));
        }
        Some(usd_vt::Value::from_no_hash(usd_vt::Array::from(arr)))
    }

    /// Matrix3d: 9 x f64 = 72 bytes per element
    fn read_raw_matrix3d_array(&self, data: &[u8], n: usize) -> Option<usd_vt::Value> {
        let elem_size = 9 * 8; // 3x3 f64
        if data.len() < n * elem_size {
            return None;
        }
        let mut arr = Vec::with_capacity(n);
        for i in 0..n {
            let base = i * elem_size;
            let mut values = [[0.0f64; 3]; 3];
            for r in 0..3 {
                for c in 0..3 {
                    let off = base + (r * 3 + c) * 8;
                    values[r][c] = f64::from_bits(u64::from_le_bytes([
                        data[off],
                        data[off + 1],
                        data[off + 2],
                        data[off + 3],
                        data[off + 4],
                        data[off + 5],
                        data[off + 6],
                        data[off + 7],
                    ]));
                }
            }
            arr.push(usd_gf::Matrix3d::from_array(values));
        }
        Some(usd_vt::Value::from_no_hash(usd_vt::Array::from(arr)))
    }

    /// Matrix4d: 16 x f64 = 128 bytes per element
    fn read_raw_matrix4d_array(&self, data: &[u8], n: usize) -> Option<usd_vt::Value> {
        let elem_size = 16 * 8; // 4x4 f64
        if data.len() < n * elem_size {
            return None;
        }
        let mut arr = Vec::with_capacity(n);
        for i in 0..n {
            let base = i * elem_size;
            let mut values = [[0.0f64; 4]; 4];
            for r in 0..4 {
                for c in 0..4 {
                    let off = base + (r * 4 + c) * 8;
                    values[r][c] = f64::from_bits(u64::from_le_bytes([
                        data[off],
                        data[off + 1],
                        data[off + 2],
                        data[off + 3],
                        data[off + 4],
                        data[off + 5],
                        data[off + 6],
                        data[off + 7],
                    ]));
                }
            }
            arr.push(usd_gf::Matrix4d::from_array(values));
        }
        Some(usd_vt::Value::from_no_hash(usd_vt::Array::from(arr)))
    }

    /// Bool array: 1 byte per element
    fn read_raw_bool_array(&self, data: &[u8], n: usize) -> Option<usd_vt::Value> {
        if data.len() < n {
            return None;
        }
        let arr: Vec<bool> = data[..n].iter().map(|&b| b != 0).collect();
        Some(usd_vt::Value::from_no_hash(usd_vt::Array::from(arr)))
    }

    // --- Compressed array readers (version >= 0.5.0 for ints, >= 0.6.0 for floats) ---

    fn read_compressed_i32_array(&self, data: &[u8], n: usize) -> Option<usd_vt::Value> {
        use crate::integer_coding::IntegerCompression;
        // Read compressedSize (u64), then decompress
        if data.len() < 8 {
            return None;
        }
        let compressed_size = u64::from_le_bytes([
            data[0], data[1], data[2], data[3], data[4], data[5], data[6], data[7],
        ]) as usize;
        if data.len() < 8 + compressed_size {
            return None;
        }
        let compressed = &data[8..8 + compressed_size];
        match IntegerCompression::decompress_i32(compressed, n) {
            Ok(arr) => Some(usd_vt::Value::from_no_hash(usd_vt::Array::from(arr))),
            Err(_) => None,
        }
    }

    fn read_compressed_u32_array(&self, data: &[u8], n: usize) -> Option<usd_vt::Value> {
        use crate::integer_coding::IntegerCompression;
        if data.len() < 8 {
            return None;
        }
        let compressed_size = u64::from_le_bytes([
            data[0], data[1], data[2], data[3], data[4], data[5], data[6], data[7],
        ]) as usize;
        if data.len() < 8 + compressed_size {
            return None;
        }
        let compressed = &data[8..8 + compressed_size];
        match IntegerCompression::decompress_u32(compressed, n) {
            Ok(arr) => Some(usd_vt::Value::from_no_hash(usd_vt::Array::from(arr))),
            Err(_) => None,
        }
    }

    fn read_compressed_i64_array(&self, data: &[u8], n: usize) -> Option<usd_vt::Value> {
        use crate::integer_coding::IntegerCompression;
        if data.len() < 8 {
            return None;
        }
        let compressed_size = u64::from_le_bytes([
            data[0], data[1], data[2], data[3], data[4], data[5], data[6], data[7],
        ]) as usize;
        if data.len() < 8 + compressed_size {
            return None;
        }
        let compressed = &data[8..8 + compressed_size];
        match IntegerCompression::decompress_i64(compressed, n) {
            Ok(arr) => Some(usd_vt::Value::from_no_hash(usd_vt::Array::from(arr))),
            Err(_) => None,
        }
    }

    fn read_compressed_u64_array(&self, data: &[u8], n: usize) -> Option<usd_vt::Value> {
        use crate::integer_coding::IntegerCompression;
        if data.len() < 8 {
            return None;
        }
        let compressed_size = u64::from_le_bytes([
            data[0], data[1], data[2], data[3], data[4], data[5], data[6], data[7],
        ]) as usize;
        if data.len() < 8 + compressed_size {
            return None;
        }
        let compressed = &data[8..8 + compressed_size];
        match IntegerCompression::decompress_u64(compressed, n) {
            Ok(arr) => Some(usd_vt::Value::from_no_hash(usd_vt::Array::from(arr))),
            Err(_) => None,
        }
    }

    /// Reads compressed float array with code byte.
    /// code 'i' = integer-compressed floats
    /// code 't' = lookup table with compressed indexes
    fn read_compressed_float_array(
        &self,
        type_enum: TypeEnum,
        data: &[u8],
        n: usize,
        code: char,
    ) -> Option<usd_vt::Value> {
        use crate::integer_coding::IntegerCompression;
        match code {
            'i' => {
                // Compressed as integers - read compressedSize, decompress ints, convert to float
                if data.len() < 8 {
                    return None;
                }
                let compressed_size = u64::from_le_bytes([
                    data[0], data[1], data[2], data[3], data[4], data[5], data[6], data[7],
                ]) as usize;
                if data.len() < 8 + compressed_size {
                    return None;
                }
                let compressed = &data[8..8 + compressed_size];
                let ints = IntegerCompression::decompress_i32(compressed, n).ok()?;
                match type_enum {
                    TypeEnum::Half => {
                        // C++ std::copy(int32_t*, GfHalf*) does numeric conversion
                        let arr: Vec<f32> = ints.iter().map(|&i| i as f32).collect();
                        Some(usd_vt::Value::from_no_hash(usd_vt::Array::from(arr)))
                    }
                    TypeEnum::Float => {
                        // C++ std::copy(int32_t*, float*) does numeric conversion
                        let arr: Vec<f32> = ints.iter().map(|&i| i as f32).collect();
                        Some(usd_vt::Value::from_no_hash(usd_vt::Array::from(arr)))
                    }
                    TypeEnum::Double => {
                        // For doubles compressed as ints, cast i32 -> f64
                        let arr: Vec<f64> = ints.iter().map(|&i| i as f64).collect();
                        Some(usd_vt::Value::from_no_hash(usd_vt::Array::from(arr)))
                    }
                    _ => None,
                }
            }
            't' => {
                // Lookup table: lutSize (u32), lut values, compressed indexes
                if data.len() < 4 {
                    return None;
                }
                let lut_size = u32::from_le_bytes([data[0], data[1], data[2], data[3]]) as usize;
                let mut offset = 4;

                // Read LUT based on type
                let elem_size = match type_enum {
                    TypeEnum::Half => 2,
                    TypeEnum::Float => 4,
                    TypeEnum::Double => 8,
                    _ => return None,
                };
                if data.len() < offset + lut_size * elem_size {
                    return None;
                }

                // Read LUT values
                let lut_data = &data[offset..offset + lut_size * elem_size];
                offset += lut_size * elem_size;

                // Read compressed indexes
                if data.len() < offset + 8 {
                    return None;
                }
                let idx_compressed_size = u64::from_le_bytes([
                    data[offset],
                    data[offset + 1],
                    data[offset + 2],
                    data[offset + 3],
                    data[offset + 4],
                    data[offset + 5],
                    data[offset + 6],
                    data[offset + 7],
                ]) as usize;
                offset += 8;
                if data.len() < offset + idx_compressed_size {
                    return None;
                }
                let idx_compressed = &data[offset..offset + idx_compressed_size];
                let indexes = IntegerCompression::decompress_u32(idx_compressed, n).ok()?;

                // Apply LUT
                match type_enum {
                    TypeEnum::Half => {
                        let lut: Vec<f32> = (0..lut_size)
                            .map(|i| {
                                let bits =
                                    u16::from_le_bytes([lut_data[i * 2], lut_data[i * 2 + 1]]);
                                half_to_f32(bits)
                            })
                            .collect();
                        let arr: Vec<f32> = indexes.iter().map(|&i| lut[i as usize]).collect();
                        Some(usd_vt::Value::from_no_hash(usd_vt::Array::from(arr)))
                    }
                    TypeEnum::Float => {
                        let lut: Vec<f32> = (0..lut_size)
                            .map(|i| {
                                let bits = u32::from_le_bytes([
                                    lut_data[i * 4],
                                    lut_data[i * 4 + 1],
                                    lut_data[i * 4 + 2],
                                    lut_data[i * 4 + 3],
                                ]);
                                f32::from_bits(bits)
                            })
                            .collect();
                        let arr: Vec<f32> = indexes.iter().map(|&i| lut[i as usize]).collect();
                        Some(usd_vt::Value::from_no_hash(usd_vt::Array::from(arr)))
                    }
                    TypeEnum::Double => {
                        let lut: Vec<f64> = (0..lut_size)
                            .map(|i| {
                                let bits = u64::from_le_bytes([
                                    lut_data[i * 8],
                                    lut_data[i * 8 + 1],
                                    lut_data[i * 8 + 2],
                                    lut_data[i * 8 + 3],
                                    lut_data[i * 8 + 4],
                                    lut_data[i * 8 + 5],
                                    lut_data[i * 8 + 6],
                                    lut_data[i * 8 + 7],
                                ]);
                                f64::from_bits(bits)
                            })
                            .collect();
                        let arr: Vec<f64> = indexes.iter().map(|&i| lut[i as usize]).collect();
                        Some(usd_vt::Value::from_no_hash(usd_vt::Array::from(arr)))
                    }
                    _ => None,
                }
            }
            _ => {
                // Unknown code - corrupt data
                None
            }
        }
    }

    // ========================================================================
    // ListOp binary format:
    // - 1 byte: header bits (isExplicit, hasExplicit, hasAdded, hasDeleted,
    //           hasOrdered, hasPrepended, hasAppended)
    // - For each present list: u64 count + count * T items
    // ========================================================================

    /// ListOp header bits
    const LIST_OP_IS_EXPLICIT: u8 = 1 << 0;
    const LIST_OP_HAS_EXPLICIT: u8 = 1 << 1;
    const LIST_OP_HAS_ADDED: u8 = 1 << 2;
    const LIST_OP_HAS_DELETED: u8 = 1 << 3;
    const LIST_OP_HAS_ORDERED: u8 = 1 << 4;
    const LIST_OP_HAS_PREPENDED: u8 = 1 << 5;
    const LIST_OP_HAS_APPENDED: u8 = 1 << 6;

    /// Reads a u64 count from data at offset, returns (count, new_offset).
    fn read_u64_count(data: &[u8], offset: usize) -> Option<(usize, usize)> {
        if data.len() < offset + 8 {
            return None;
        }
        let count = u64::from_le_bytes([
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
            data[offset + 4],
            data[offset + 5],
            data[offset + 6],
            data[offset + 7],
        ]) as usize;
        Some((count, offset + 8))
    }

    /// Reads a vector of tokens from file data.
    fn read_token_vector(
        &self,
        data: &[u8],
        offset: usize,
        crate_file: &CrateFile,
    ) -> Option<(Vec<Token>, usize)> {
        let (count, mut off) = Self::read_u64_count(data, offset)?;
        let mut items = Vec::with_capacity(count);
        for _ in 0..count {
            if data.len() < off + 4 {
                return None;
            }
            let idx = u32::from_le_bytes([data[off], data[off + 1], data[off + 2], data[off + 3]]);
            off += 4;
            let token_idx = TokenIndex::new(idx);
            if let Some(t) = crate_file.get_token(token_idx) {
                items.push(t.clone());
            } else {
                return None;
            }
        }
        Some((items, off))
    }

    /// Reads a vector of strings from file data (string table indices).
    /// C++ stores each string as a u32 StringIndex, resolved via crate string table.
    fn read_string_vector_items(
        &self,
        data: &[u8],
        offset: usize,
        crate_file: &CrateFile,
    ) -> Option<(Vec<String>, usize)> {
        let (count, mut off) = Self::read_u64_count(data, offset)?;
        let mut items = Vec::with_capacity(count);
        for _ in 0..count {
            // Each element is a u32 StringIndex into the crate string table
            if data.len() < off + 4 {
                return None;
            }
            let idx = u32::from_le_bytes([data[off], data[off + 1], data[off + 2], data[off + 3]]);
            off += 4;
            let str_idx = StringIndex::new(idx);
            if let Some(s) = crate_file.get_string(str_idx) {
                items.push(s.to_string());
            } else {
                return None;
            }
        }
        Some((items, off))
    }

    /// Reads a vector of paths from file data (path indices).
    fn read_path_vector_items(
        &self,
        data: &[u8],
        offset: usize,
        crate_file: &CrateFile,
    ) -> Option<(Vec<crate::Path>, usize)> {
        let (count, mut off) = Self::read_u64_count(data, offset)?;
        let mut items = Vec::with_capacity(count);
        for _ in 0..count {
            if data.len() < off + 4 {
                return None;
            }
            let idx = u32::from_le_bytes([data[off], data[off + 1], data[off + 2], data[off + 3]])
                as usize;
            off += 4;
            if idx < crate_file.paths.len() {
                items.push(crate_file.paths[idx].clone());
            } else {
                return None;
            }
        }
        Some((items, off))
    }

    /// Reads a vector of integers from file data.
    fn read_int_vector<T: FromLeBytes + Default + Clone>(
        &self,
        data: &[u8],
        offset: usize,
    ) -> Option<(Vec<T>, usize)> {
        let (count, mut off) = Self::read_u64_count(data, offset)?;
        let elem_size = std::mem::size_of::<T>();
        if data.len() < off + count * elem_size {
            return None;
        }
        let mut items = Vec::with_capacity(count);
        for _ in 0..count {
            items.push(T::from_le_bytes(&data[off..]));
            off += elem_size;
        }
        Some((items, off))
    }

    /// Unpacks TokenListOp from file data.
    fn unpack_token_list_op(&self, data: &[u8], crate_file: &CrateFile) -> Option<usd_vt::Value> {
        if data.is_empty() {
            return None;
        }
        let bits = data[0];
        let mut offset = 1;
        let mut list_op = super::ListOp::<Token>::new();

        if bits & Self::LIST_OP_IS_EXPLICIT != 0 {
            list_op.clear_and_make_explicit();
        }
        if bits & Self::LIST_OP_HAS_EXPLICIT != 0 {
            let (items, off) = self.read_token_vector(data, offset, crate_file)?;
            offset = off;
            list_op.set_explicit_items(items).ok();
        }
        if bits & Self::LIST_OP_HAS_ADDED != 0 {
            let (items, off) = self.read_token_vector(data, offset, crate_file)?;
            offset = off;
            list_op.set_added_items(items);
        }
        if bits & Self::LIST_OP_HAS_PREPENDED != 0 {
            let (items, off) = self.read_token_vector(data, offset, crate_file)?;
            offset = off;
            list_op.set_prepended_items(items).ok();
        }
        if bits & Self::LIST_OP_HAS_APPENDED != 0 {
            let (items, off) = self.read_token_vector(data, offset, crate_file)?;
            offset = off;
            list_op.set_appended_items(items).ok();
        }
        if bits & Self::LIST_OP_HAS_DELETED != 0 {
            let (items, off) = self.read_token_vector(data, offset, crate_file)?;
            offset = off;
            list_op.set_deleted_items(items).ok();
        }
        if bits & Self::LIST_OP_HAS_ORDERED != 0 {
            let (items, _off) = self.read_token_vector(data, offset, crate_file)?;
            list_op.set_ordered_items(items);
        }

        Some(usd_vt::Value::from_no_hash(list_op))
    }

    /// Unpacks StringListOp from file data.
    fn unpack_string_list_op(&self, data: &[u8], crate_file: &CrateFile) -> Option<usd_vt::Value> {
        if data.is_empty() {
            return None;
        }
        let bits = data[0];
        let mut offset = 1;
        let mut list_op = super::ListOp::<String>::new();

        if bits & Self::LIST_OP_IS_EXPLICIT != 0 {
            list_op.clear_and_make_explicit();
        }
        if bits & Self::LIST_OP_HAS_EXPLICIT != 0 {
            let (items, off) = self.read_string_vector_items(data, offset, crate_file)?;
            offset = off;
            list_op.set_explicit_items(items).ok();
        }
        if bits & Self::LIST_OP_HAS_ADDED != 0 {
            let (items, off) = self.read_string_vector_items(data, offset, crate_file)?;
            offset = off;
            list_op.set_added_items(items);
        }
        if bits & Self::LIST_OP_HAS_PREPENDED != 0 {
            let (items, off) = self.read_string_vector_items(data, offset, crate_file)?;
            offset = off;
            list_op.set_prepended_items(items).ok();
        }
        if bits & Self::LIST_OP_HAS_APPENDED != 0 {
            let (items, off) = self.read_string_vector_items(data, offset, crate_file)?;
            offset = off;
            list_op.set_appended_items(items).ok();
        }
        if bits & Self::LIST_OP_HAS_DELETED != 0 {
            let (items, off) = self.read_string_vector_items(data, offset, crate_file)?;
            offset = off;
            list_op.set_deleted_items(items).ok();
        }
        if bits & Self::LIST_OP_HAS_ORDERED != 0 {
            let (items, _off) = self.read_string_vector_items(data, offset, crate_file)?;
            list_op.set_ordered_items(items);
        }

        Some(usd_vt::Value::from_no_hash(list_op))
    }

    /// Unpacks PathListOp from file data.
    fn unpack_path_list_op(&self, data: &[u8], crate_file: &CrateFile) -> Option<usd_vt::Value> {
        if data.is_empty() {
            return None;
        }
        let bits = data[0];
        let mut offset = 1;
        let mut list_op = super::ListOp::<crate::Path>::new();

        if bits & Self::LIST_OP_IS_EXPLICIT != 0 {
            list_op.clear_and_make_explicit();
        }
        if bits & Self::LIST_OP_HAS_EXPLICIT != 0 {
            let (items, off) = self.read_path_vector_items(data, offset, crate_file)?;
            offset = off;
            list_op.set_explicit_items(items).ok();
        }
        if bits & Self::LIST_OP_HAS_ADDED != 0 {
            let (items, off) = self.read_path_vector_items(data, offset, crate_file)?;
            offset = off;
            list_op.set_added_items(items);
        }
        if bits & Self::LIST_OP_HAS_PREPENDED != 0 {
            let (items, off) = self.read_path_vector_items(data, offset, crate_file)?;
            offset = off;
            list_op.set_prepended_items(items).ok();
        }
        if bits & Self::LIST_OP_HAS_APPENDED != 0 {
            let (items, off) = self.read_path_vector_items(data, offset, crate_file)?;
            offset = off;
            list_op.set_appended_items(items).ok();
        }
        if bits & Self::LIST_OP_HAS_DELETED != 0 {
            let (items, off) = self.read_path_vector_items(data, offset, crate_file)?;
            offset = off;
            list_op.set_deleted_items(items).ok();
        }
        if bits & Self::LIST_OP_HAS_ORDERED != 0 {
            let (items, _off) = self.read_path_vector_items(data, offset, crate_file)?;
            list_op.set_ordered_items(items);
        }

        Some(usd_vt::Value::from_no_hash(list_op))
    }

    /// Unpacks IntListOp (i32, u32, i64, u64) from file data.
    fn unpack_int_list_op<T>(&self, data: &[u8]) -> Option<usd_vt::Value>
    where
        T: Clone
            + Eq
            + std::hash::Hash
            + std::fmt::Debug
            + FromLeBytes
            + Default
            + Send
            + Sync
            + PartialEq
            + 'static,
    {
        if data.is_empty() {
            return None;
        }
        let bits = data[0];
        let mut offset = 1;
        let mut list_op = super::ListOp::<T>::new();

        if bits & Self::LIST_OP_IS_EXPLICIT != 0 {
            list_op.clear_and_make_explicit();
        }
        if bits & Self::LIST_OP_HAS_EXPLICIT != 0 {
            let (items, off) = self.read_int_vector::<T>(data, offset)?;
            offset = off;
            list_op.set_explicit_items(items).ok();
        }
        if bits & Self::LIST_OP_HAS_ADDED != 0 {
            let (items, off) = self.read_int_vector::<T>(data, offset)?;
            offset = off;
            list_op.set_added_items(items);
        }
        if bits & Self::LIST_OP_HAS_PREPENDED != 0 {
            let (items, off) = self.read_int_vector::<T>(data, offset)?;
            offset = off;
            list_op.set_prepended_items(items).ok();
        }
        if bits & Self::LIST_OP_HAS_APPENDED != 0 {
            let (items, off) = self.read_int_vector::<T>(data, offset)?;
            offset = off;
            list_op.set_appended_items(items).ok();
        }
        if bits & Self::LIST_OP_HAS_DELETED != 0 {
            let (items, off) = self.read_int_vector::<T>(data, offset)?;
            offset = off;
            list_op.set_deleted_items(items).ok();
        }
        if bits & Self::LIST_OP_HAS_ORDERED != 0 {
            let (items, _off) = self.read_int_vector::<T>(data, offset)?;
            list_op.set_ordered_items(items);
        }

        Some(usd_vt::Value::from_no_hash(list_op))
    }

    /// Unpacks ReferenceListOp from file data.
    /// Reference format: assetPath (string) + primPath (path index) + layerOffset + customData
    fn unpack_reference_list_op(
        &self,
        data: &[u8],
        crate_file: &CrateFile,
    ) -> Option<usd_vt::Value> {
        if data.is_empty() {
            return None;
        }
        let bits = data[0];
        let mut offset = 1;
        let mut list_op = super::ListOp::<super::Reference>::new();

        let read_refs =
            |data: &[u8], off: usize, cf: &CrateFile| -> Option<(Vec<super::Reference>, usize)> {
                let (count, mut o) = Self::read_u64_count(data, off)?;
                let mut refs = Vec::with_capacity(count);
                for _ in 0..count {
                    // Read assetPath (string index)
                    if data.len() < o + 4 {
                        return None;
                    }
                    let asset_idx =
                        u32::from_le_bytes([data[o], data[o + 1], data[o + 2], data[o + 3]]);
                    o += 4;
                    let asset_path_str = cf
                        .get_string(StringIndex::new(asset_idx))
                        .map(|s| s.to_string())
                        .unwrap_or_default();

                    // Read primPath (path index)
                    if data.len() < o + 4 {
                        return None;
                    }
                    let path_idx =
                        u32::from_le_bytes([data[o], data[o + 1], data[o + 2], data[o + 3]])
                            as usize;
                    o += 4;
                    let prim_path_str = if path_idx < cf.paths.len() {
                        cf.paths[path_idx].to_string()
                    } else {
                        String::new()
                    };

                    // Read layerOffset (scale + offset as f64s)
                    if data.len() < o + 16 {
                        return None;
                    }
                    let layer_offset_val = f64::from_bits(u64::from_le_bytes([
                        data[o],
                        data[o + 1],
                        data[o + 2],
                        data[o + 3],
                        data[o + 4],
                        data[o + 5],
                        data[o + 6],
                        data[o + 7],
                    ]));
                    o += 8;
                    let layer_scale = f64::from_bits(u64::from_le_bytes([
                        data[o],
                        data[o + 1],
                        data[o + 2],
                        data[o + 3],
                        data[o + 4],
                        data[o + 5],
                        data[o + 6],
                        data[o + 7],
                    ]));
                    o += 8;
                    let layer_offset = super::LayerOffset::new(layer_offset_val, layer_scale);

                    // Read customData (VtDictionary) to stay stream-aligned (P1-7)
                    // C++ writes: u64 count + per-entry (StringIndex u32 + ValueRep u64)
                    if data.len() < o + 8 {
                        return None;
                    }
                    let custom_data_count = u64::from_le_bytes([
                        data[o],
                        data[o + 1],
                        data[o + 2],
                        data[o + 3],
                        data[o + 4],
                        data[o + 5],
                        data[o + 6],
                        data[o + 7],
                    ]) as usize;
                    o += 8;
                    // Skip past entries: each is StringIndex(u32) + ValueRep(u64) = 12 bytes
                    let custom_data_bytes = custom_data_count * 12;
                    if data.len() < o + custom_data_bytes {
                        return None;
                    }
                    o += custom_data_bytes;

                    let reference = super::Reference::with_metadata(
                        asset_path_str,
                        &prim_path_str,
                        layer_offset,
                        Default::default(),
                    );
                    refs.push(reference);
                }
                Some((refs, o))
            };

        if bits & Self::LIST_OP_IS_EXPLICIT != 0 {
            list_op.clear_and_make_explicit();
        }
        if bits & Self::LIST_OP_HAS_EXPLICIT != 0 {
            let (items, off) = read_refs(data, offset, crate_file)?;
            offset = off;
            list_op.set_explicit_items(items).ok();
        }
        if bits & Self::LIST_OP_HAS_ADDED != 0 {
            let (items, off) = read_refs(data, offset, crate_file)?;
            offset = off;
            list_op.set_added_items(items);
        }
        if bits & Self::LIST_OP_HAS_PREPENDED != 0 {
            let (items, off) = read_refs(data, offset, crate_file)?;
            offset = off;
            list_op.set_prepended_items(items).ok();
        }
        if bits & Self::LIST_OP_HAS_APPENDED != 0 {
            let (items, off) = read_refs(data, offset, crate_file)?;
            offset = off;
            list_op.set_appended_items(items).ok();
        }
        if bits & Self::LIST_OP_HAS_DELETED != 0 {
            let (items, off) = read_refs(data, offset, crate_file)?;
            offset = off;
            list_op.set_deleted_items(items).ok();
        }
        if bits & Self::LIST_OP_HAS_ORDERED != 0 {
            let (items, _off) = read_refs(data, offset, crate_file)?;
            list_op.set_ordered_items(items);
        }

        Some(usd_vt::Value::from_no_hash(list_op))
    }

    /// Unpacks PayloadListOp from file data.
    /// Payload format: assetPath + primPath + layerOffset (version >= 0.8.0)
    fn unpack_payload_list_op(&self, data: &[u8], crate_file: &CrateFile) -> Option<usd_vt::Value> {
        if data.is_empty() {
            return None;
        }
        let bits = data[0];
        let mut offset = 1;
        let mut list_op = super::ListOp::<super::Payload>::new();
        // C++: layerOffset was added to SdfPayload in version 0.8.0
        let can_read_layer_offset = crate_file.version >= (0, 8, 0);

        let read_payloads = |data: &[u8],
                             off: usize,
                             cf: &CrateFile,
                             has_layer_offset: bool|
         -> Option<(Vec<super::Payload>, usize)> {
            let (count, mut o) = Self::read_u64_count(data, off)?;
            let mut payloads = Vec::with_capacity(count);
            for _ in 0..count {
                // Read assetPath
                if data.len() < o + 4 {
                    return None;
                }
                let asset_idx =
                    u32::from_le_bytes([data[o], data[o + 1], data[o + 2], data[o + 3]]);
                o += 4;
                let asset_path_str = cf
                    .get_string(StringIndex::new(asset_idx))
                    .map(|s| s.to_string())
                    .unwrap_or_default();

                // Read primPath
                if data.len() < o + 4 {
                    return None;
                }
                let path_idx =
                    u32::from_le_bytes([data[o], data[o + 1], data[o + 2], data[o + 3]]) as usize;
                o += 4;
                let prim_path_str = if path_idx < cf.paths.len() {
                    cf.paths[path_idx].to_string()
                } else {
                    String::new()
                };

                // Read layerOffset only for version >= 0.8.0
                let payload = if has_layer_offset {
                    if data.len() < o + 16 {
                        return None;
                    }
                    let layer_offset_val = f64::from_bits(u64::from_le_bytes([
                        data[o],
                        data[o + 1],
                        data[o + 2],
                        data[o + 3],
                        data[o + 4],
                        data[o + 5],
                        data[o + 6],
                        data[o + 7],
                    ]));
                    o += 8;
                    let layer_scale = f64::from_bits(u64::from_le_bytes([
                        data[o],
                        data[o + 1],
                        data[o + 2],
                        data[o + 3],
                        data[o + 4],
                        data[o + 5],
                        data[o + 6],
                        data[o + 7],
                    ]));
                    o += 8;
                    let layer_offset = super::LayerOffset::new(layer_offset_val, layer_scale);
                    super::Payload::with_layer_offset(asset_path_str, &prim_path_str, layer_offset)
                } else {
                    super::Payload::new(asset_path_str, &prim_path_str)
                };
                payloads.push(payload);
            }
            Some((payloads, o))
        };

        if bits & Self::LIST_OP_IS_EXPLICIT != 0 {
            list_op.clear_and_make_explicit();
        }
        if bits & Self::LIST_OP_HAS_EXPLICIT != 0 {
            let (items, off) = read_payloads(data, offset, crate_file, can_read_layer_offset)?;
            offset = off;
            list_op.set_explicit_items(items).ok();
        }
        if bits & Self::LIST_OP_HAS_ADDED != 0 {
            let (items, off) = read_payloads(data, offset, crate_file, can_read_layer_offset)?;
            offset = off;
            list_op.set_added_items(items);
        }
        if bits & Self::LIST_OP_HAS_PREPENDED != 0 {
            let (items, off) = read_payloads(data, offset, crate_file, can_read_layer_offset)?;
            offset = off;
            list_op.set_prepended_items(items).ok();
        }
        if bits & Self::LIST_OP_HAS_APPENDED != 0 {
            let (items, off) = read_payloads(data, offset, crate_file, can_read_layer_offset)?;
            offset = off;
            list_op.set_appended_items(items).ok();
        }
        if bits & Self::LIST_OP_HAS_DELETED != 0 {
            let (items, off) = read_payloads(data, offset, crate_file, can_read_layer_offset)?;
            offset = off;
            list_op.set_deleted_items(items).ok();
        }
        if bits & Self::LIST_OP_HAS_ORDERED != 0 {
            let (items, _off) = read_payloads(data, offset, crate_file, can_read_layer_offset)?;
            list_op.set_ordered_items(items);
        }

        Some(usd_vt::Value::from_no_hash(list_op))
    }

    // ========================================================================
    // Vector types (PathVector, TokenVector, DoubleVector, StringVector)
    // ========================================================================

    /// Unpacks PathVector from file data.
    fn unpack_path_vector(&self, data: &[u8], crate_file: &CrateFile) -> Option<usd_vt::Value> {
        let (items, _) = self.read_path_vector_items(data, 0, crate_file)?;
        Some(usd_vt::Value::from_no_hash(items))
    }

    /// Unpacks TokenVector from file data.
    fn unpack_token_vector(&self, data: &[u8], crate_file: &CrateFile) -> Option<usd_vt::Value> {
        let (items, _) = self.read_token_vector(data, 0, crate_file)?;
        Some(usd_vt::Value::from_no_hash(items))
    }

    /// Unpacks DoubleVector from file data.
    fn unpack_double_vector(&self, data: &[u8]) -> Option<usd_vt::Value> {
        let (items, _) = self.read_int_vector::<f64>(data, 0)?;
        Some(usd_vt::Value::from_no_hash(items))
    }

    /// Unpacks StringVector from file data.
    fn unpack_string_vector(&self, data: &[u8], crate_file: &CrateFile) -> Option<usd_vt::Value> {
        let (items, _) = self.read_string_vector_items(data, 0, crate_file)?;
        Some(usd_vt::Value::from_no_hash(items))
    }

    /// Gets the usda format for string operations.
    fn get_usda_format() -> Option<Arc<dyn FileFormat>> {
        find_format_by_id(&usda_reader::tokens::id())
    }
}

impl FileFormat for UsdcFileFormat {
    fn format_id(&self) -> Token {
        self.format_id.clone()
    }

    fn target(&self) -> Token {
        self.target.clone()
    }

    fn file_extensions(&self) -> Vec<String> {
        self.extensions.clone()
    }

    fn can_read(&self, path: &str) -> bool {
        CrateData::can_read(path)
    }

    fn read(
        &self,
        layer: &mut Layer,
        resolved_path: &ResolvedPath,
        metadata_only: bool,
    ) -> Result<(), FileFormatError> {
        self.read_helper(layer, resolved_path.as_str(), metadata_only, false)
    }

    fn write_to_file(
        &self,
        layer: &Layer,
        file_path: &str,
        _comment: Option<&str>,
        _args: &FileFormatArguments,
    ) -> Result<(), FileFormatError> {
        // Use current version (0.8.0 is commonly used)
        let mut writer = CrateWriter::new((0, 8, 0));

        // Populate writer from layer
        writer.populate_from_layer(layer);

        // Write the crate file
        let data = writer.write();

        // Write to file
        std::fs::write(file_path, data)
            .map_err(|e| FileFormatError::io_error(file_path.to_string(), e.to_string()))
    }

    /// Reads from string by delegating to usda format.
    ///
    /// Binary data cannot be meaningfully represented as a string,
    /// so string operations use the text format.
    fn write_to_string(
        &self,
        layer: &Layer,
        comment: Option<&str>,
    ) -> Result<String, FileFormatError> {
        // Delegate to usda format
        if let Some(usda) = Self::get_usda_format() {
            usda.write_to_string(layer, comment)
        } else {
            Err(FileFormatError::other("usda format not available"))
        }
    }

    fn get_file_cookie(&self) -> String {
        String::from_utf8_lossy(USDC_MAGIC).to_string()
    }

    fn get_version_string(&self) -> Token {
        self.version_string.clone()
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
}

// ============================================================================
// Additional Methods
// ============================================================================

impl UsdcFileFormat {
    /// Reads layer from raw bytes.
    ///
    /// This is useful for reading USDC files from memory (e.g., from USDZ archives).
    pub fn read_from_bytes_impl(
        &self,
        layer: &mut Layer,
        data: &[u8],
        _metadata_only: bool,
    ) -> Result<(), FileFormatError> {
        // Parse crate file
        let crate_file = CrateFile::open(data, "<memory>")?;

        // Populate layer from crate file
        self.populate_layer_from_crate(layer, &crate_file, data)?;

        Ok(())
    }

    /// Reads layer from string content.
    ///
    /// Delegates to usda format since binary data cannot be in string form.
    pub fn read_from_string(
        &self,
        _layer: &mut Layer,
        _content: &str,
    ) -> Result<(), FileFormatError> {
        // Binary format does not support string input
        Err(FileFormatError::other(
            "Binary format does not support string input",
        ))
    }

    /// Writes a spec to a stream.
    ///
    /// Delegates to usda format.
    pub fn write_to_stream(
        &self,
        _spec: &super::Spec,
        _output: &mut dyn Write,
        _indent: usize,
    ) -> Result<(), FileFormatError> {
        // Spec writing not implemented yet
        Ok(())
    }

    /// Reads in detached mode.
    pub fn read_detached(
        &self,
        layer: &mut Layer,
        resolved_path: &ResolvedPath,
        metadata_only: bool,
    ) -> Result<(), FileFormatError> {
        self.read_helper(layer, resolved_path.as_str(), metadata_only, true)
    }

    /// Saves to file (may preserve format-specific state).
    pub fn save_to_file(
        &self,
        layer: &Layer,
        file_path: &str,
        _comment: Option<&str>,
        _args: &FileFormatArguments,
    ) -> Result<(), FileFormatError> {
        // Check if layer has crate-backed data and use Save instead of Export
        // For now, same as write_to_file
        self.write_to_file(layer, file_path, None, &FileFormatArguments::new())
    }
}

// ============================================================================
// Public API functions
// ============================================================================

/// Reads a layer from raw USDC bytes.
///
/// This is useful for reading USDC files from memory (e.g., from USDZ archives).
///
/// # Arguments
///
/// * `layer` - Layer to populate with the parsed data
/// * `data` - Raw USDC file bytes
/// * `metadata_only` - If true, only read metadata (not full hierarchy)
///
/// # Errors
///
/// Returns error if the data is not valid USDC format.
pub fn read_from_bytes(
    layer: &mut Layer,
    data: &[u8],
    metadata_only: bool,
) -> Result<(), FileFormatError> {
    let format = UsdcFileFormat::new();
    format.read_from_bytes_impl(layer, data, metadata_only)
}

// ============================================================================
// Registration
// ============================================================================

/// Registers the usdc file format globally.
pub fn register_usdc_format() {
    register_file_format(Arc::new(UsdcFileFormat::new()));
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_usdc_format_id() {
        let format = UsdcFileFormat::new();
        assert_eq!(format.format_id(), Token::new("usdc"));
        assert_eq!(format.target(), Token::new("usd"));
    }

    #[test]
    fn test_usdc_extensions() {
        let format = UsdcFileFormat::new();
        assert_eq!(format.file_extensions(), vec!["usdc".to_string()]);
    }

    #[test]
    fn test_usdc_capabilities() {
        let format = UsdcFileFormat::new();
        assert!(format.supports_reading());
        assert!(format.supports_writing());
        assert!(format.supports_editing());
        assert!(!format.is_package());
    }

    #[test]
    fn test_can_read_bytes() {
        assert!(CrateData::can_read_bytes(b"PXR-USDC\x00\x09\x00"));
        assert!(!CrateData::can_read_bytes(b"#usda 1.0"));
        assert!(!CrateData::can_read_bytes(b"PK\x03\x04"));
    }

    #[test]
    fn test_section_type_conversion() {
        assert_eq!(SectionType::from(1), SectionType::Tokens);
        assert_eq!(SectionType::from(2), SectionType::Strings);
        assert_eq!(SectionType::from(3), SectionType::Fields);
        assert_eq!(SectionType::from(4), SectionType::FieldSets);
        assert_eq!(SectionType::from(5), SectionType::Paths);
        assert_eq!(SectionType::from(6), SectionType::Specs);
        assert_eq!(SectionType::from(255), SectionType::Unknown);
    }

    #[test]
    fn test_crate_header_roundtrip() {
        let header = CrateHeader {
            version_major: 0,
            version_minor: 9,
            version_patch: 0,
            num_sections: 6,
            toc_offset: 1024,
        };

        let bytes = header.to_bytes();
        // CrateHeader.to_bytes() produces Bootstrap format (88 bytes)
        assert_eq!(bytes.len(), BOOTSTRAP_SIZE);

        let parsed = CrateHeader::from_bytes(&bytes).unwrap();
        assert_eq!(parsed.version(), (0, 9, 0));
        // num_sections is not stored in bootstrap, it's derived from ToC
        assert_eq!(parsed.toc_offset, 1024);
    }

    #[test]
    fn test_header_invalid_magic() {
        let data = b"INVALID-xxxxxxxxxxxxxxxx";
        let result = CrateHeader::from_bytes(data);
        assert!(result.is_err());
    }

    #[test]
    fn test_file_cookie() {
        let format = UsdcFileFormat::new();
        assert_eq!(format.get_file_cookie(), "PXR-USDC");
    }

    #[test]
    fn test_version_string() {
        let format = UsdcFileFormat::new();
        assert_eq!(format.get_version_string(), Token::new("0.9.0"));
    }

    #[test]
    fn test_software_version_token() {
        let token = CrateData::software_version_token();
        assert_eq!(token, Token::new("0.9.0"));
    }

    #[test]
    fn test_crate_data_new() {
        let data = CrateData::new(false);
        assert!(!data.is_detached());

        let detached = CrateData::new(true);
        assert!(detached.is_detached());
    }

    #[test]
    fn test_type_enum_values() {
        assert_eq!(TypeEnum::Invalid as i32, 0);
        assert_eq!(TypeEnum::Bool as i32, 1);
        assert_eq!(TypeEnum::Double as i32, 9);
        assert_eq!(TypeEnum::Token as i32, 11);
        assert_eq!(TypeEnum::Matrix4d as i32, 15);
        assert_eq!(TypeEnum::Vec3f as i32, 24);
        assert_eq!(TypeEnum::Dictionary as i32, 31);
        assert_eq!(TypeEnum::TimeSamples as i32, 46);
        assert_eq!(TypeEnum::AnimationBlock as i32, 60);
    }

    #[test]
    fn test_type_enum_from_raw() {
        assert_eq!(TypeEnum::from_raw(0), TypeEnum::Invalid);
        assert_eq!(TypeEnum::from_raw(1), TypeEnum::Bool);
        assert_eq!(TypeEnum::from_raw(9), TypeEnum::Double);
        assert_eq!(TypeEnum::from_raw(24), TypeEnum::Vec3f);
        assert_eq!(TypeEnum::from_raw(60), TypeEnum::AnimationBlock);
        assert_eq!(TypeEnum::from_raw(999), TypeEnum::Invalid);
    }

    #[test]
    fn test_type_enum_supports_array() {
        assert!(TypeEnum::Bool.supports_array());
        assert!(TypeEnum::Double.supports_array());
        assert!(TypeEnum::Vec3f.supports_array());
        assert!(TypeEnum::Matrix4d.supports_array());
        assert!(TypeEnum::Token.supports_array());
        assert!(TypeEnum::TimeCode.supports_array());

        assert!(!TypeEnum::Dictionary.supports_array());
        assert!(!TypeEnum::TimeSamples.supports_array());
        assert!(!TypeEnum::Specifier.supports_array());
        assert!(!TypeEnum::ValueBlock.supports_array());
    }

    #[test]
    fn test_value_rep_construction() {
        let rep = ValueRep::new(TypeEnum::Double, true, false, 42);
        assert_eq!(rep.get_type(), TypeEnum::Double);
        assert!(rep.is_inlined());
        assert!(!rep.is_array());
        assert_eq!(rep.get_payload(), 42);
    }

    #[test]
    fn test_value_rep_array() {
        let rep = ValueRep::new(TypeEnum::Vec3f, false, true, 1024);
        assert_eq!(rep.get_type(), TypeEnum::Vec3f);
        assert!(!rep.is_inlined());
        assert!(rep.is_array());
        assert_eq!(rep.get_payload(), 1024);
    }

    #[test]
    fn test_value_rep_flags() {
        let mut rep = ValueRep::default();
        assert!(!rep.is_array());
        assert!(!rep.is_inlined());
        assert!(!rep.is_compressed());
        assert!(!rep.is_array_edit());

        rep.set_is_array();
        assert!(rep.is_array());

        rep.set_is_inlined();
        assert!(rep.is_inlined());

        rep.set_is_compressed();
        assert!(rep.is_compressed());

        rep.set_is_array_edit();
        assert!(rep.is_array_edit());
    }

    #[test]
    fn test_value_rep_type_modification() {
        let mut rep = ValueRep::new(TypeEnum::Int, true, false, 100);
        assert_eq!(rep.get_type(), TypeEnum::Int);

        rep.set_type(TypeEnum::Float);
        assert_eq!(rep.get_type(), TypeEnum::Float);
        assert_eq!(rep.get_payload(), 100); // payload unchanged
    }

    #[test]
    fn test_value_rep_payload_modification() {
        let mut rep = ValueRep::new(TypeEnum::Token, false, false, 500);
        assert_eq!(rep.get_payload(), 500);

        rep.set_payload(1000);
        assert_eq!(rep.get_payload(), 1000);
        assert_eq!(rep.get_type(), TypeEnum::Token); // type unchanged
    }

    #[test]
    fn test_value_rep_roundtrip() {
        let original = ValueRep::new(TypeEnum::Matrix4d, false, true, 0x123456789ABC);
        let bytes = original.to_bytes();
        let parsed = ValueRep::from_bytes(&bytes).unwrap();
        assert_eq!(original, parsed);
    }

    #[test]
    fn test_index_types() {
        let idx = Index::new(42);
        assert_eq!(idx.value, 42);
        assert!(idx.is_valid());

        let invalid = Index::invalid();
        assert!(!invalid.is_valid());
        assert_eq!(invalid.value, u32::MAX);
    }

    #[test]
    fn test_field_index_types() {
        let field_idx = FieldIndex::new(10);
        assert_eq!(field_idx.0.value, 10);

        let invalid = FieldIndex::invalid();
        assert!(!invalid.0.is_valid());
    }

    #[test]
    fn test_field_roundtrip() {
        let field = Field::new(
            TokenIndex::new(5),
            ValueRep::new(TypeEnum::String, false, false, 1000),
        );
        let bytes = field.to_bytes();
        let parsed = Field::from_bytes(&bytes).unwrap();
        assert_eq!(field.token_index, parsed.token_index);
        assert_eq!(field.value_rep, parsed.value_rep);
    }

    #[test]
    fn test_crate_spec_roundtrip() {
        let spec = CrateSpec::new(PathIndex::new(3), FieldSetIndex::new(7), SpecType::Prim);
        let bytes = spec.to_bytes();
        let parsed = CrateSpec::from_bytes(&bytes).unwrap();
        assert_eq!(spec.path_index.0.value, parsed.path_index.0.value);
        assert_eq!(spec.field_set_index.0.value, parsed.field_set_index.0.value);
        assert_eq!(spec.spec_type, parsed.spec_type);
    }

    #[test]
    fn test_crate_time_samples() {
        let ts = CrateTimeSamples::new();
        assert!(ts.is_in_memory());
        assert!(ts.is_empty());
        assert_eq!(ts.len(), 0);
    }

    #[test]
    fn test_bootstrap_roundtrip() {
        let bootstrap = Bootstrap::with_version(0, 9, 0);
        let bytes = bootstrap.to_bytes();
        assert_eq!(bytes.len(), BOOTSTRAP_SIZE);

        let parsed = Bootstrap::from_bytes(&bytes).unwrap();
        assert_eq!(parsed.version_tuple(), (0, 9, 0));
        assert_eq!(&parsed.ident, USDC_MAGIC);
    }

    #[test]
    fn test_section_roundtrip() {
        let section = Section::new("TOKENS", 1024, 512);
        let bytes = section.to_bytes();
        assert_eq!(bytes.len(), Section::SIZE);

        let parsed = Section::from_bytes(&bytes).unwrap();
        assert_eq!(parsed.name, "TOKENS");
        assert_eq!(parsed.start, 1024);
        assert_eq!(parsed.size, 512);
    }

    #[test]
    fn test_table_of_contents() {
        let mut toc = TableOfContents::new();
        toc.sections.push(Section::new("TOKENS", 100, 50));
        toc.sections.push(Section::new("STRINGS", 150, 30));
        toc.sections.push(Section::new("FIELDS", 180, 100));

        assert_eq!(toc.get_section("TOKENS").unwrap().start, 100);
        assert_eq!(toc.get_section("STRINGS").unwrap().start, 150);
        assert!(toc.get_section("NONEXISTENT").is_none());
        assert_eq!(toc.min_section_start(), 100);
    }

    #[test]
    fn test_crate_file_new() {
        let cf = CrateFile::new();
        assert_eq!(cf.version, SOFTWARE_VERSION);
        assert!(!cf.detached);
        assert!(cf.tokens.is_empty());
        assert!(cf.paths.is_empty());
        assert!(cf.specs.is_empty());
    }

    #[test]
    fn test_crate_file_detached() {
        let cf = CrateFile::new_detached();
        assert!(cf.detached);
    }

    #[test]
    fn test_crate_file_can_read() {
        assert!(CrateFile::can_read(b"PXR-USDC\x00\x09\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00"));
        assert!(!CrateFile::can_read(b"#usda 1.0"));
        assert!(!CrateFile::can_read(b"PXR-USD")); // Too short
    }

    #[test]
    fn test_crate_file_num_field_sets() {
        let mut cf = CrateFile::new();
        // Add some field indices with terminators
        cf.field_sets.push(FieldIndex::new(0));
        cf.field_sets.push(FieldIndex::new(1));
        cf.field_sets.push(FieldIndex::invalid()); // Terminator 1
        cf.field_sets.push(FieldIndex::new(2));
        cf.field_sets.push(FieldIndex::invalid()); // Terminator 2

        assert_eq!(cf.num_unique_field_sets(), 2);
    }

    #[test]
    fn test_read_real_usdc_file() {
        // Try to read a real USDC file from the reference repo
        let test_file = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/testenv/usdc/a.usdc"
        );

        if !std::path::Path::new(test_file).exists() {
            // Skip if reference repo not available
            return;
        }

        let data = std::fs::read(test_file).expect("Failed to read test file");
        assert!(!data.is_empty());

        // Verify it's a valid USDC file
        assert!(CrateFile::can_read(&data));

        // Try to open it
        let crate_file = CrateFile::open(&data, test_file);
        assert!(crate_file.is_ok(), "Failed to open: {:?}", crate_file.err());

        let cf = crate_file.unwrap();

        // Check that we read something
        println!("Version: {:?}", cf.version);
        println!("Tokens: {}", cf.tokens.len());
        println!("Paths: {}", cf.paths.len());
        println!("Fields: {}", cf.fields.len());
        println!("Specs: {}", cf.specs.len());

        assert!(cf.tokens.len() > 0, "Should have some tokens");
        assert!(cf.paths.len() > 0, "Should have some paths");
    }

    #[test]
    fn test_read_teapot_usdc() {
        // Read teapot mesh USDC file (version 0.8.0 with compressed paths)
        let test_file = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/testenv/usdc/teapot.usdc"
        );

        if !std::path::Path::new(test_file).exists() {
            return;
        }

        let data = std::fs::read(test_file).expect("Failed to read test file");
        assert!(CrateFile::can_read(&data));

        let crate_file = CrateFile::open(&data, test_file);
        assert!(
            crate_file.is_ok(),
            "Failed to open teapot: {:?}",
            crate_file.err()
        );

        let cf = crate_file.unwrap();

        println!("Teapot Version: {:?}", cf.version);
        println!("Teapot Tokens: {}", cf.tokens.len());
        println!("Teapot Paths: {}", cf.paths.len());
        println!("Teapot Fields: {}", cf.fields.len());
        println!("Teapot Specs: {}", cf.specs.len());

        // Teapot should have mesh data
        assert!(cf.tokens.len() > 10, "Should have many tokens");
        assert!(cf.paths.len() > 1, "Should have multiple paths");
        assert!(cf.fields.len() > 5, "Should have many fields");

        // Print some paths for inspection
        println!("Paths:");
        for (i, path) in cf.paths.iter().take(10).enumerate() {
            println!("  {}: {}", i, path);
        }
    }

    #[test]
    fn test_dictionary_unpack_from_usdc() {
        // Read a real USDC file that may contain Dictionary-typed metadata.
        // BMW X3 or Audi models contain material customData / sdrMetadata dicts.
        let candidates = [
            concat!(env!("CARGO_MANIFEST_DIR"), "/../../../data/bmw_x3.usdc"),
            concat!(env!("CARGO_MANIFEST_DIR"), "/../../../data/audi.usdc"),
        ];

        let test_file = candidates.iter().find(|p| std::path::Path::new(p).exists());
        let Some(test_file) = test_file else {
            // No data files available in CI — skip gracefully.
            return;
        };

        let data = std::fs::read(test_file).expect("Failed to read USDC file");
        let crate_file = CrateFile::open(&data, test_file).expect("Failed to open USDC file");

        // Collect Dictionary-typed fields from the crate.
        let mut dict_count = 0usize;
        let format = UsdcFileFormat::new();

        for field in &crate_file.fields {
            let rep = &field.value_rep;
            if rep.get_type() == TypeEnum::Dictionary {
                let value = format.unpack_value(rep, &crate_file, &data);
                if let Some(v) = value {
                    if let Some(dict) = v.get::<usd_vt::Dictionary>() {
                        println!("Dictionary field: {} keys", dict.len());
                        for (k, _) in dict.iter() {
                            println!("  key: {}", k);
                        }
                        dict_count += 1;
                    }
                }
            }
        }

        println!("Total Dictionary fields found: {}", dict_count);
        // We just verify parsing didn't panic; actual count depends on the file.
        // dict_count >= 0 is always true but the real check is no panics above.
        let _ = dict_count;
    }

    /// Verifies `unpack_dictionary` with a hand-crafted binary payload:
    ///   count=2, then two (StringIndex u32, ValueRep u64) pairs.
    ///   Keys are inlined strings; values are inlined ints (TypeEnum::Int).
    #[test]
    fn test_unpack_dictionary_synthetic() {
        use super::*;

        // Build a minimal CrateFile with two tokens used as strings.
        let mut cf = CrateFile::default();
        // Strings are stored via tokens in our implementation.
        // CrateFile.strings is Vec<TokenIndex>, CrateFile.tokens is Vec<Token>.
        // We need: get_string(StringIndex(0)) => "alpha", get_string(StringIndex(1)) => "beta".
        let tok_alpha = TokenIndex::new(0);
        let tok_beta = TokenIndex::new(1);
        cf.tokens.push(Token::new("alpha"));
        cf.tokens.push(Token::new("beta"));
        cf.strings.push(tok_alpha);
        cf.strings.push(tok_beta);

        // Value reps: inlined Int with values 42 and 99.
        // ValueRep layout: upper byte = TypeEnum, IS_INLINED_BIT set, lower 32 bits = payload.
        //   IS_INLINED_BIT = bit 62.  TypeEnum::Int = 1.
        //   data = (TypeEnum as u64) << 48 | IS_INLINED_BIT | (payload as u64)
        let make_inlined_int = |v: u32| -> u64 {
            let type_byte = TypeEnum::Int as u64;
            type_byte << 48 | ValueRep::IS_INLINED_BIT | v as u64
        };

        // Binary payload for unpack_dictionary (offset points here):
        //   [0..8]    u64 count = 2
        //   [8..12]   u32 StringIndex(0) = "alpha"
        //   [12..20]  u64 ValueRep(inlined Int 42)
        //   [20..24]  u32 StringIndex(1) = "beta"
        //   [24..32]  u64 ValueRep(inlined Int 99)
        let mut payload = Vec::<u8>::new();
        payload.extend_from_slice(&2u64.to_le_bytes()); // count
        payload.extend_from_slice(&0u32.to_le_bytes()); // key "alpha"
        payload.extend_from_slice(&make_inlined_int(42).to_le_bytes()); // value 42
        payload.extend_from_slice(&1u32.to_le_bytes()); // key "beta"
        payload.extend_from_slice(&make_inlined_int(99).to_le_bytes()); // value 99

        let format = UsdcFileFormat::new();
        let result = format.unpack_dictionary(0, &cf, &payload);

        assert!(result.is_some(), "unpack_dictionary should return Some");
        let val = result.unwrap();
        let dict = val
            .get::<usd_vt::Dictionary>()
            .expect("value should hold Dictionary");

        assert_eq!(dict.len(), 2, "Dictionary should have 2 entries");
        assert_eq!(dict.get_as::<i32>("alpha"), Some(&42));
        assert_eq!(dict.get_as::<i32>("beta"), Some(&99));
    }

    /// Diagnostic test: write a simple CrateWriter output and parse it back.
    #[test]
    fn test_writer_basic_roundtrip() {
        let mut writer = CrateWriter::new((0, 8, 0));

        // Add paths manually (empty path always at index 0)
        let root = Path::absolute_root();
        let world = root.append_child("World").unwrap();
        let world_mesh = world.append_child("Mesh").unwrap();
        writer.add_path(&root);
        writer.add_path(&world);
        writer.add_path(&world_mesh);

        // Add a simple spec for each
        let root_idx = *writer.path_to_index.get(&root).unwrap();
        let root_field_set = writer.add_field_set(&[]);
        writer.add_spec(root_idx, SpecType::PseudoRoot, root_field_set);

        let world_idx = *writer.path_to_index.get(&world).unwrap();
        let world_field_set = writer.add_field_set(&[]);
        writer.add_spec(world_idx, SpecType::Prim, world_field_set);

        let mesh_idx = *writer.path_to_index.get(&world_mesh).unwrap();
        let mesh_field_set = writer.add_field_set(&[]);
        writer.add_spec(mesh_idx, SpecType::Prim, mesh_field_set);

        let data = writer.write();
        eprintln!("Written {} bytes", data.len());

        // Read it back
        let crate_file = CrateFile::open(&data, "<test>");
        assert!(crate_file.is_ok(), "Failed to open: {:?}", crate_file.err());
        let cf = crate_file.unwrap();
        eprintln!("Paths read: {}", cf.paths.len());
        for (i, p) in cf.paths.iter().enumerate() {
            eprintln!("  path[{}] = {:?}", i, p.to_string());
        }
        // Expect: path[0]="" (empty), path[1]="/", path[2]="/World", path[3]="/World/Mesh"
        assert_eq!(cf.paths.len(), 4, "Expected 4 paths (empty + 3 real)");
        assert_eq!(cf.paths[1].to_string(), "/");
        assert_eq!(cf.paths[2].to_string(), "/World");
        assert_eq!(cf.paths[3].to_string(), "/World/Mesh");
    }

    /// End-to-end: write Layer with float attribute → USDC bytes → CrateFile parse → verify field.
    #[test]
    fn test_layer_float_attr_roundtrip() {
        use super::super::Layer;

        // Build a layer in memory
        let layer = Layer::create_anonymous(None);
        let root_path = Path::from_string("/Root").unwrap();
        let prim_path = Path::from_string("/Root/Mesh").unwrap();
        layer.create_prim_spec(&root_path, super::super::Specifier::Def, "");
        layer.create_prim_spec(&prim_path, super::super::Specifier::Def, "Mesh");

        let attr_path = prim_path.append_property("val").unwrap();
        layer.create_spec(&attr_path, SpecType::Attribute);

        let field_name = usd_tf::Token::new("default");
        layer.set_field(&attr_path, &field_name, usd_vt::Value::from(1.23_f32));

        // Serialize to USDC
        let mut writer = CrateWriter::new((0, 8, 0));
        writer.populate_from_layer(&layer);
        let data = writer.write();

        eprintln!("USDC data: {} bytes", data.len());

        // Open the USDC via CrateFile
        let cf = CrateFile::open(&data, "<test>").expect("CrateFile::open failed");
        eprintln!("Tokens ({}):", cf.tokens.len());
        for (i, t) in cf.tokens.iter().enumerate() {
            eprintln!("  token[{}] = {:?}", i, t.get_text());
        }
        eprintln!("Paths ({}):", cf.paths.len());
        for (i, p) in cf.paths.iter().enumerate() {
            eprintln!("  path[{}] = {:?}", i, p.to_string());
        }
        eprintln!("Specs ({}):", cf.specs.len());
        for s in &cf.specs {
            let p = cf
                .get_path(s.path_index)
                .map(|p| p.to_string())
                .unwrap_or_default();
            eprintln!(
                "  spec path={} type={:?} fieldset={}",
                p, s.spec_type, s.field_set_index.0.value
            );
        }
        eprintln!("Fields ({}):", cf.fields.len());
        for (i, f) in cf.fields.iter().enumerate() {
            let tok = cf
                .get_token(f.token_index)
                .map(|t| t.get_text().to_string())
                .unwrap_or_default();
            eprintln!(
                "  field[{}] name={:?} rep=0x{:016x}",
                i, tok, f.value_rep.data
            );
        }

        // Find the "default" field for the attribute spec
        let attr_spec = cf.specs.iter().find(|s| {
            cf.get_path(s.path_index)
                .map(|p| p.to_string() == attr_path.to_string())
                .unwrap_or(false)
        });
        assert!(
            attr_spec.is_some(),
            "Attribute spec not found for {:?}",
            attr_path.to_string()
        );
        let attr_spec = attr_spec.unwrap();

        // Find "default" field in this spec's fieldset
        let fmt = UsdcFileFormat::new();
        let field_set_start = attr_spec.field_set_index.0.value as usize;
        let mut found_default = false;
        let mut field_idx = field_set_start;
        while field_idx < cf.field_sets.len() {
            let fi = &cf.field_sets[field_idx];
            if !fi.0.is_valid() {
                break;
            }
            if let Some(field) = cf.get_field(*fi) {
                if let Some(tok) = cf.get_token(field.token_index) {
                    if tok.get_text() == "default" {
                        eprintln!("Found 'default' field: rep=0x{:016x}", field.value_rep.data);
                        let v = fmt.unpack_value(&field.value_rep, &cf, &data);
                        eprintln!("Unpacked: {:?}", v.as_ref().map(|x| format!("{:?}", x)));
                        assert!(v.is_some(), "'default' value unpacked as None");
                        let fval = v.unwrap();
                        if let Some(&f) = fval.get::<f32>() {
                            assert!((f - 1.23_f32).abs() < 1e-5, "Expected ~1.23 got {}", f);
                            eprintln!("float val = {} OK", f);
                        } else {
                            panic!("Expected f32, got: {:?}", fval);
                        }
                        found_default = true;
                        break;
                    }
                }
            }
            field_idx += 1;
        }
        assert!(found_default, "'default' field not found in attribute spec");
    }

    /// Diagnostic: dump xformOp-related specs and fields from audi.usdc.
    /// Run with: cargo test -p usd-sdf test_audi_xform_diagnostic -- --nocapture
    #[test]
    fn test_audi_xform_diagnostic() {
        let candidates = [
            concat!(env!("CARGO_MANIFEST_DIR"), "/../../../data/audi.usdc"),
            concat!(env!("CARGO_MANIFEST_DIR"), "/../../../data/bmw_x3.usdc"),
        ];
        let test_file = candidates.iter().find(|p| std::path::Path::new(p).exists());
        let Some(test_file) = test_file else {
            return; // skip if no file
        };

        let data = std::fs::read(test_file).expect("read usdc");
        let cf = CrateFile::open(&data, test_file).expect("open usdc");
        let fmt = UsdcFileFormat::new();

        println!("File: {} version {:?}", test_file, cf.version);

        // Find xformOp and xformOpOrder related specs/fields
        let mut printed = 0;
        for spec in &cf.specs {
            let path_str = cf
                .get_path(spec.path_index)
                .map(|p| p.to_string())
                .unwrap_or_default();

            // Look for xformOp paths
            if !path_str.contains("xformOp") && !path_str.contains("xformOpOrder") {
                continue;
            }
            if printed > 30 {
                break;
            }
            printed += 1;

            println!("\nSpec: path={} type={:?}", path_str, spec.spec_type);

            // Dump fields for this spec
            let fs_start = spec.field_set_index.0.value as usize;
            let mut fi = fs_start;
            while fi < cf.field_sets.len() {
                let fidx = &cf.field_sets[fi];
                if !fidx.0.is_valid() {
                    break;
                }
                if let Some(field) = cf.get_field(*fidx) {
                    let fname = cf
                        .get_token(field.token_index)
                        .map(|t| t.as_str().to_string())
                        .unwrap_or_default();
                    let type_e = field.value_rep.get_type();
                    let inlined = field.value_rep.is_inlined();
                    let is_arr = field.value_rep.is_array();
                    println!(
                        "  field={:?} type={:?} inlined={} array={} rep=0x{:016x}",
                        fname, type_e, inlined, is_arr, field.value_rep.data
                    );

                    // Try to unpack
                    if let Some(v) = fmt.unpack_value(&field.value_rep, &cf, &data) {
                        let type_name = v.type_name();
                        println!("    => unpacked type_name={:?}", type_name);
                        // Print Vec3d value if possible
                        if let Some(vec) = v.get::<usd_gf::Vec3d>() {
                            println!("    => Vec3d({}, {}, {})", vec.x, vec.y, vec.z);
                        }
                        // Print Vec3f value if possible
                        if let Some(vec) = v.get::<usd_gf::Vec3f>() {
                            println!("    => Vec3f({}, {}, {})", vec.x, vec.y, vec.z);
                        }
                        // Print Matrix4d translation row if possible
                        if let Some(mat) = v.get::<usd_gf::Matrix4d>() {
                            println!(
                                "    => Matrix4d row3=({}, {}, {}, {})",
                                mat[3][0], mat[3][1], mat[3][2], mat[3][3]
                            );
                        }
                        // Print token array
                        if let Some(toks) = v.get::<Vec<usd_tf::Token>>() {
                            println!(
                                "    => Token[{}]: {:?}",
                                toks.len(),
                                toks.iter().take(5).map(|t| t.as_str()).collect::<Vec<_>>()
                            );
                        }
                    } else {
                        println!("    => unpack returned None");
                    }
                }
                fi += 1;
            }
        }
        // Also scan prim specs for xformOpOrder field
        println!("\n--- Prim specs with xformOpOrder field ---");
        let xform_order_tok = "xformOpOrder";
        let mut count = 0;
        'outer: for spec in &cf.specs {
            let fs_start = spec.field_set_index.0.value as usize;
            let mut fi = fs_start;
            while fi < cf.field_sets.len() {
                let fidx = &cf.field_sets[fi];
                if !fidx.0.is_valid() {
                    break;
                }
                if let Some(field) = cf.get_field(*fidx) {
                    if cf
                        .get_token(field.token_index)
                        .map(|t| t == xform_order_tok)
                        .unwrap_or(false)
                    {
                        let path_str = cf
                            .get_path(spec.path_index)
                            .map(|p| p.to_string())
                            .unwrap_or_default();
                        println!(
                            "  prim={} field={} type={:?} rep=0x{:016x}",
                            path_str,
                            xform_order_tok,
                            field.value_rep.get_type(),
                            field.value_rep.data
                        );
                        if let Some(v) = fmt.unpack_value(&field.value_rep, &cf, &data) {
                            if let Some(toks) = v.get::<Vec<usd_tf::Token>>() {
                                println!(
                                    "    tokens={:?}",
                                    toks.iter().take(8).map(|t| t.as_str()).collect::<Vec<_>>()
                                );
                            }
                        }
                        count += 1;
                        if count >= 5 {
                            break 'outer;
                        }
                    }
                }
                fi += 1;
            }
        }
    }

    /// Diagnostic: check all time samples in flo.usdc via Layer API.
    #[test]
    fn diag_flo_time_samples_layer() {
        let test_file = concat!(env!("CARGO_MANIFEST_DIR"), "/../../../data/flo.usdc");
        if !std::path::Path::new(test_file).exists() {
            eprintln!("SKIP: flo.usdc not found");
            return;
        }
        // Register file formats so Layer::find_or_open can resolve .usdc
        crate::init();
        let layer = Layer::find_or_open(test_file).expect("open flo.usdc");

        // --- 1. Traverse all specs, collect paths ---
        let root = Path::absolute_root();
        let ts_token = Token::new("timeSamples");
        let collected = std::cell::RefCell::new(Vec::<Path>::new());
        layer.traverse(&root, &|p: &Path| {
            collected.borrow_mut().push(p.clone());
        });
        let all_paths = collected.into_inner();
        eprintln!("\n=== flo.usdc via Layer API ===");
        eprintln!("Total specs traversed: {}", all_paths.len());

        // --- 2. Per-path: time samples + raw field check ---
        let mut paths_with_ts: Vec<(String, Vec<f64>)> = Vec::new();
        let mut paths_with_raw_field: Vec<String> = Vec::new();
        for path in &all_paths {
            let times = layer.list_time_samples_for_path(path);
            if !times.is_empty() {
                paths_with_ts.push((path.to_string(), times));
            }
            if layer.get_field(path, &ts_token).is_some() {
                paths_with_raw_field.push(path.to_string());
            }
        }

        // --- 3. Report paths with time samples ---
        eprintln!("\nPATHS WITH TIME SAMPLES: {}", paths_with_ts.len());
        let mut xform_count = 0usize;
        let mut other_count = 0usize;
        for (p, times) in &paths_with_ts {
            let n = times.len().min(5);
            if p.contains("xformOp") {
                xform_count += 1;
                if xform_count <= 15 {
                    eprintln!(
                        "  XFORM {} => {} samples, first={:?}",
                        p,
                        times.len(),
                        &times[..n]
                    );
                    let path_obj = all_paths.iter().find(|x| x.to_string() == *p).unwrap();
                    if let Some(t0) = times.first() {
                        match layer.query_time_sample(path_obj, *t0) {
                            Some(val) => {
                                eprintln!("    sample[t={}] type={:?}", t0, val.type_name())
                            }
                            None => eprintln!("    sample[t={}] => NONE (bug!)", t0),
                        }
                    }
                }
            } else {
                other_count += 1;
                if other_count <= 10 {
                    eprintln!(
                        "  OTHER {} => {} samples, first={:?}",
                        p,
                        times.len(),
                        &times[..n]
                    );
                }
            }
        }
        eprintln!("  xformOp: {}, other: {}", xform_count, other_count);

        // --- 4. Global distinct times ---
        let global_times = layer.list_all_time_samples();
        eprintln!(
            "\nlist_all_time_samples(): {} distinct times",
            global_times.len()
        );
        if !global_times.is_empty() {
            let n = global_times.len().min(5);
            let tail = global_times.len().saturating_sub(5);
            eprintln!("  first 5: {:?}", &global_times[..n]);
            eprintln!("  last  5: {:?}", &global_times[tail..]);
        }

        // --- 5. Raw timeSamples field (should be 0 if properly expanded) ---
        eprintln!(
            "\nRAW 'timeSamples' plain field count (should be 0): {}",
            paths_with_raw_field.len()
        );
        for p in paths_with_raw_field.iter().take(10) {
            eprintln!("  {}", p);
        }

        // --- 6. Layer time metadata ---
        let tcps_tok = Token::new("timeCodesPerSecond");
        let start_tok = Token::new("startTimeCode");
        let end_tok = Token::new("endTimeCode");
        eprintln!("\nLAYER METADATA:");
        eprintln!(
            "  timeCodesPerSecond: {:?}",
            layer
                .get_field(&root, &tcps_tok)
                .map(|v| format!("{:?}", v.type_name()))
        );
        eprintln!(
            "  startTimeCode:      {:?}",
            layer
                .get_field(&root, &start_tok)
                .map(|v| format!("{:?}", v.type_name()))
        );
        eprintln!(
            "  endTimeCode:        {:?}",
            layer
                .get_field(&root, &end_tok)
                .map(|v| format!("{:?}", v.type_name()))
        );
        eprintln!("  start_time_code() = {:?}", layer.start_time_code());
        eprintln!("  end_time_code()   = {:?}", layer.end_time_code());

        // Always passes — output is the diagnostic payload
        assert!(true, "diagnostic complete");
    }
    /// Diagnostic test for flo.usdc points geometry — research only.
    /// Scans ALL specs (no SpecType filter) for .points path suffix or 'points' field name.
    /// Also reports spec-type distribution to reveal what types exist in the file.
    #[test]
    fn test_flo_points_diagnostic() {
        let test_file = concat!(env!("CARGO_MANIFEST_DIR"), "/../../../data/flo.usdc");
        if !std::path::Path::new(test_file).exists() {
            return;
        }
        let data = std::fs::read(test_file).expect("read usdc");
        let cf = CrateFile::open(&data, test_file).expect("open usdc");
        let fmt = UsdcFileFormat::new();
        println!("File: {} version {:?}", test_file, cf.version);
        println!("Total specs: {}", cf.specs.len());

        // 1. Spec-type distribution (to understand what types are used)
        let mut type_counts: std::collections::HashMap<String, usize> = Default::default();
        for spec in &cf.specs {
            *type_counts
                .entry(format!("{:?}", spec.spec_type))
                .or_insert(0) += 1;
        }
        let mut tv: Vec<_> = type_counts.into_iter().collect();
        tv.sort_by_key(|(k, _)| k.clone());
        println!("Spec type distribution:");
        for (t, c) in &tv {
            println!("  {:?} = {}", t, c);
        }

        // 2. Scan all specs — find those whose resolved path ends with ".points"
        println!("\n--- Specs with path ending in .points (any SpecType) ---");
        let mut count = 0;
        for spec in &cf.specs {
            let path_str = cf
                .get_path(spec.path_index)
                .map(|p| p.to_string())
                .unwrap_or_default();
            if !path_str.ends_with(".points") {
                continue;
            }
            count += 1;
            if count > 5 {
                break;
            }
            println!("  Spec: path={} spec_type={:?}", path_str, spec.spec_type);
            let fs_start = spec.field_set_index.0.value as usize;
            let mut fi = fs_start;
            while fi < cf.field_sets.len() {
                let fidx = &cf.field_sets[fi];
                if !fidx.0.is_valid() {
                    break;
                }
                if let Some(field) = cf.get_field(*fidx) {
                    let fname = cf
                        .get_token(field.token_index)
                        .map(|t| t.as_str().to_string())
                        .unwrap_or_default();
                    let type_e = field.value_rep.get_type();
                    let is_arr = field.value_rep.is_array();
                    println!("    field={} type={:?} array={}", fname, type_e, is_arr);
                    if fname == "default" || fname == "timeSamples" {
                        if let Some(v) = fmt.unpack_value(&field.value_rep, &cf, &data) {
                            println!("      => rust_type={:?}", v.type_name());
                            // Use as_vec_clone which handles both Vec<T> and Array<T>
                            if let Some(pts) = v.as_vec_clone::<usd_gf::Vec3f>() {
                                let p = &pts[0];
                                println!(
                                    "      => Vec3f[{}] via as_vec_clone: ({}, {}, {}), ...",
                                    pts.len(),
                                    p.x,
                                    p.y,
                                    p.z
                                );
                            } else if let Some(pts) = v.as_vec_clone::<usd_gf::Vec3d>() {
                                let p = &pts[0];
                                println!(
                                    "      => Vec3d[{}] via as_vec_clone: ({}, {}, {}), ...",
                                    pts.len(),
                                    p.x,
                                    p.y,
                                    p.z
                                );
                            } else {
                                println!("      => (as_vec_clone returned None — unexpected)");
                            }
                        } else {
                            println!("      => unpack returned None");
                        }
                    }
                }
                fi += 1;
            }
        }
        if count == 0 {
            println!("  (none — path suffix .points not found in any spec)");
        }

        // 3. Scan all specs — find those that have a field NAMED 'points'
        //    (Blender may store mesh data as prim properties, not child AttributeSpecs)
        println!("\n--- Prim specs that have a field named 'points' ---");
        let mut mesh_count = 0;
        let mut seen: std::collections::HashSet<usize> = Default::default();
        for (si, spec) in cf.specs.iter().enumerate() {
            if !seen.insert(si) {
                continue;
            }
            let fs_start = spec.field_set_index.0.value as usize;
            let mut fi = fs_start;
            let mut found_field: Option<ValueRep> = None;
            while fi < cf.field_sets.len() {
                let fidx = &cf.field_sets[fi];
                if !fidx.0.is_valid() {
                    break;
                }
                if let Some(field) = cf.get_field(*fidx) {
                    let fname = cf
                        .get_token(field.token_index)
                        .map(|t| t.as_str().to_string())
                        .unwrap_or_default();
                    if fname == "points" {
                        found_field = Some(field.value_rep);
                        break;
                    }
                }
                fi += 1;
            }
            let Some(vrep) = found_field else { continue };
            let path_str = cf
                .get_path(spec.path_index)
                .map(|p| p.to_string())
                .unwrap_or_default();
            mesh_count += 1;
            if mesh_count > 3 {
                break;
            }
            let type_e = vrep.get_type();
            let is_arr = vrep.is_array();
            println!(
                "  Spec: path={} spec_type={:?} points_field: type={:?} array={}",
                path_str, spec.spec_type, type_e, is_arr
            );
            if let Some(v) = fmt.unpack_value(&vrep, &cf, &data) {
                println!("    => rust_type={:?}", v.type_name());
                if let Some(pts) = v.get::<Vec<usd_gf::Vec3f>>() {
                    let p = &pts[0];
                    println!(
                        "    => Vec3f[{}]: ({}, {}, {}), ...",
                        pts.len(),
                        p.x,
                        p.y,
                        p.z
                    );
                } else if let Some(pts) = v.get::<Vec<usd_gf::Vec3d>>() {
                    let p = &pts[0];
                    println!(
                        "    => Vec3d[{}]: ({}, {}, {}), ...",
                        pts.len(),
                        p.x,
                        p.y,
                        p.z
                    );
                } else {
                    println!("    => (cannot downcast)");
                }
            }
        }
        if mesh_count == 0 {
            println!("  (none — no spec has a field named 'points')");
        }
    }

    /// Diagnostic test for flo.usdc xformOp transforms — research only.
    #[test]
    fn test_flo_xform_diagnostic() {
        let test_file = concat!(env!("CARGO_MANIFEST_DIR"), "/../../../data/flo.usdc");
        if !std::path::Path::new(test_file).exists() {
            return;
        }
        let data = std::fs::read(test_file).expect("read usdc");
        let cf = CrateFile::open(&data, test_file).expect("open usdc");
        let fmt = UsdcFileFormat::new();
        println!("File: {} version {:?}", test_file, cf.version);

        // Find the first few xformOpOrder specs specifically
        let mut order_count = 0;
        for spec in &cf.specs {
            let path_str = cf
                .get_path(spec.path_index)
                .map(|p| p.to_string())
                .unwrap_or_default();
            if !path_str.contains("xformOpOrder") {
                continue;
            }
            if order_count >= 5 {
                break;
            }
            order_count += 1;
            println!("\nSpec: path={} type={:?}", path_str, spec.spec_type);
            let fs_start = spec.field_set_index.0.value as usize;
            let mut fi = fs_start;
            while fi < cf.field_sets.len() {
                let fidx = &cf.field_sets[fi];
                if !fidx.0.is_valid() {
                    break;
                }
                if let Some(field) = cf.get_field(*fidx) {
                    let fname = cf
                        .get_token(field.token_index)
                        .map(|t| t.as_str().to_string())
                        .unwrap_or_default();
                    let type_e = field.value_rep.get_type();
                    let inlined = field.value_rep.is_inlined();
                    let is_arr = field.value_rep.is_array();
                    println!(
                        "  field={:?} type={:?} inlined={} array={} rep=0x{:016x}",
                        fname, type_e, inlined, is_arr, field.value_rep.data
                    );
                    if let Some(v) = fmt.unpack_value(&field.value_rep, &cf, &data) {
                        let type_name = v.type_name();
                        println!("    => unpacked type_name={:?}", type_name);
                        if let Some(toks) = v.get::<Vec<usd_tf::Token>>() {
                            println!(
                                "    => Token[{}]: {:?}",
                                toks.len(),
                                toks.iter().take(8).map(|t| t.as_str()).collect::<Vec<_>>()
                            );
                        }
                    } else {
                        println!("    => unpack returned None");
                    }
                }
                fi += 1;
            }
        }
        // Find xformOp:translate for a prim that has xformOpOrder
        let mut tr_count = 0;
        for spec in &cf.specs {
            let path_str = cf
                .get_path(spec.path_index)
                .map(|p| p.to_string())
                .unwrap_or_default();
            if !path_str.contains("xformOp:translate") {
                continue;
            }
            if tr_count >= 3 {
                break;
            }
            tr_count += 1;
            println!("\nSpec: path={} type={:?}", path_str, spec.spec_type);
            let fs_start = spec.field_set_index.0.value as usize;
            let mut fi = fs_start;
            while fi < cf.field_sets.len() {
                let fidx = &cf.field_sets[fi];
                if !fidx.0.is_valid() {
                    break;
                }
                if let Some(field) = cf.get_field(*fidx) {
                    let fname = cf
                        .get_token(field.token_index)
                        .map(|t| t.as_str().to_string())
                        .unwrap_or_default();
                    let type_e = field.value_rep.get_type();
                    let inlined = field.value_rep.is_inlined();
                    let is_arr = field.value_rep.is_array();
                    println!(
                        "  field={:?} type={:?} inlined={} array={} rep=0x{:016x}",
                        fname, type_e, inlined, is_arr, field.value_rep.data
                    );
                    if let Some(v) = fmt.unpack_value(&field.value_rep, &cf, &data) {
                        let type_name = v.type_name();
                        println!("    => unpacked type_name={:?}", type_name);
                        if let Some(vec) = v.get::<usd_gf::Vec3d>() {
                            println!("    => Vec3d({}, {}, {})", vec.x, vec.y, vec.z);
                        }
                        if let Some(vec) = v.get::<usd_gf::Vec3f>() {
                            println!("    => Vec3f({}, {}, {})", vec.x, vec.y, vec.z);
                        }
                    } else {
                        println!("    => unpack returned None");
                    }
                }
                fi += 1;
            }
        }
    }

    /// Verifies that animated (time-sampled) Matrix4d values round-trip through the
    /// USDC writer + reader with the correct C++-compatible binary layout.
    ///
    /// Regression test for two bugs that were fixed together:
    /// - Writer: missing `jump2` wrapper before `numValues`, value blobs written after
    ///   `numValues` instead of before it.
    /// - Reader: read `jump1` as `timesRep`, read times blob header as `numValues`, pointed
    ///   `values_offset` into the middle of the times blob.
    ///
    /// Tests pack_time_samples + unpack_time_samples directly, bypassing populate_from_layer
    /// to avoid AttributeSpec wiring complexity.
    #[test]
    fn test_time_samples_roundtrip_matrix4d() {
        use usd_gf::Matrix4d;

        // Build expected (time, Matrix4d) pairs.
        let mut expected: Vec<(f64, Matrix4d)> = Vec::new();
        for frame in 0..5_u32 {
            let t = frame as f64;
            let mut m = Matrix4d::identity();
            m[3][0] = t * 10.0;
            m[3][1] = t * 20.0;
            m[3][2] = t * 30.0;
            expected.push((t, m));
        }

        // Pack via writer (directly calling pack_time_samples through a minimal writer).
        let mut writer = CrateWriter::new((0, 8, 0));
        let samples: std::collections::HashMap<crate::attribute_spec::OrderedFloat, usd_vt::Value> =
            expected
                .iter()
                .map(|(t, m)| {
                    (
                        crate::attribute_spec::OrderedFloat::new(*t),
                        usd_vt::Value::from_no_hash(*m),
                    )
                })
                .collect();

        let ts_rep = writer.pack_time_samples(&samples);

        // Finalise a minimal crate file so we have an address space to unpack from.
        // We need to embed the writer buffer into a full USDC file because unpack_value
        // uses absolute file offsets stored inside the ValueRep payload.
        //
        // Strategy: add a dummy token/path/spec so write() produces a valid file,
        // then search the resulting bytes for a ValueRep with TypeEnum::TimeSamples.
        let _ = ts_rep; // already encoded into writer.buffer at the right offset

        // Bake the buffer into a self-contained USDC file.
        let usdc_bytes = writer.write();

        // Parse as a CrateFile to locate the TimeSamples value rep we packed.
        // The ValueRep payload = start_offset returned by pack_time_samples.
        // We can directly construct the rep from the known offset.
        let fmt = UsdcFileFormat::new();
        // ts_rep has type=TimeSamples and payload=start_offset within the data section.
        // After write() the data section is embedded in usdc_bytes; the payload is an
        // absolute byte offset into usdc_bytes.
        let unpacked = fmt.unpack_value(&ts_rep, &CrateFile::new(), &usdc_bytes);
        let unpacked = unpacked.unwrap_or_else(|| {
            panic!(
                "unpack_value returned None for TimeSamples rep=0x{:016x}",
                ts_rep.data
            )
        });

        let ts = unpacked
            .get::<CrateTimeSamples>()
            .expect("timeSamples unpacked to wrong type");

        assert_eq!(
            ts.times.len(),
            expected.len(),
            "Expected {} time keys, got {}",
            expected.len(),
            ts.times.len()
        );
        assert_eq!(
            ts.values.len(),
            expected.len(),
            "Expected {} sample values, got {}",
            expected.len(),
            ts.values.len()
        );

        for (i, (exp_t, exp_mat)) in expected.iter().enumerate() {
            let got_t = ts.times[i];
            assert!(
                (exp_t - got_t).abs() < 1e-12,
                "Time[{i}] mismatch: expected {exp_t} got {got_t}"
            );

            let got_val = &ts.values[i];
            let got_mat = got_val
                .get::<Matrix4d>()
                .unwrap_or_else(|| panic!("sample[{i}] is not Matrix4d"));
            for r in 0..4 {
                for c in 0..4 {
                    assert!(
                        (exp_mat[r][c] - got_mat[r][c]).abs() < 1e-12,
                        "Matrix4d[{r}][{c}] mismatch at sample {i}: expected {} got {}",
                        exp_mat[r][c],
                        got_mat[r][c]
                    );
                }
            }
        }
    }

    #[test]
    fn test_value_rep_payload_zero_is_empty_array() {
        // C++ CrateFile::UnpackArray: payload==0 means empty array, NOT file offset 0.
        // Regression: previously payload==0 read from file offset 0 = "PXR-USDC" header.
        use super::types::ValueRep;

        // IsArray=true, type=Token (0x0b), payload=0
        let rep = ValueRep::from_raw(0x800b_0000_0000_0000);
        assert!(rep.is_array(), "must be array");
        assert!(!rep.is_inlined(), "must not be inlined");
        assert_eq!(rep.get_payload(), 0, "payload must be 0");

        // Verify via UsdcFileFormat::unpack_value — uses the payload==0 guard.
        let fmt = UsdcFileFormat::new();
        let cf = CrateFile::default();
        let fake_file_data = b"PXR-USDC\x00\x09\x00\x00\x00\x00\x00\x00";
        let result = fmt.unpack_value(&rep, &cf, fake_file_data);
        // Must be Some(empty typed array), not a panic or garbage data.
        // C++ returns an empty VtArray<T> for payload==0, not nullptr.
        let val = result.expect("payload==0 array must return Some(empty array)");
        // Verify it's an empty Token array
        if let Some(arr) = val.downcast::<usd_vt::Array<usd_tf::Token>>() {
            assert!(arr.is_empty(), "payload==0 array must be empty");
        } else {
            panic!("expected Array<Token>, got {:?}", val.type_name());
        }
    }

    /// Benchmark: USDC parse phases for bmw_x3.usdc.
    /// Run with: cargo test -p usd-sdf bench_usdc_parse -- --nocapture --ignored
    #[allow(unsafe_code)]
    #[test]
    #[ignore]
    fn bench_usdc_parse_bmw() {
        let workspace =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../data/bmw_x3.usdc");
        let file_data = std::fs::read(&workspace)
            .expect("bmw_x3.usdc not found -- place in data/ at workspace root");
        eprintln!("File size: {:.1} MB", file_data.len() as f64 / 1_048_576.0);

        // Warm up
        let _ = CrateFile::open(&file_data, "bmw_x3.usdc");

        let iters = 3;
        let mut struct_times = Vec::new();
        let mut populate_times = Vec::new();

        for i in 0..iters {
            // Phase 1: structural parse (header, tokens, paths, specs, fields)
            let t0 = std::time::Instant::now();
            let crate_file = CrateFile::open(&file_data, "bmw_x3.usdc").unwrap();
            let struct_ms = t0.elapsed().as_millis();
            struct_times.push(struct_ms);

            // Phase 2: populate layer (unpack values, build SDF layer)
            let t1 = std::time::Instant::now();
            let layer = Layer::create_anonymous(Some(".usdc"));
            let layer_mut = unsafe { &mut *(Arc::as_ptr(&layer) as *mut Layer) };
            let fmt = UsdcFileFormat::new();
            fmt.populate_layer_from_crate(layer_mut, &crate_file, &file_data)
                .unwrap();
            let pop_ms = t1.elapsed().as_millis();
            populate_times.push(pop_ms);

            eprintln!(
                "  iter {}: struct={}ms populate={}ms total={}ms",
                i,
                struct_ms,
                pop_ms,
                struct_ms + pop_ms
            );
        }

        let avg_s: u128 = struct_times.iter().sum::<u128>() / iters;
        let avg_p: u128 = populate_times.iter().sum::<u128>() / iters;
        eprintln!(
            "AVG: struct={}ms populate={}ms total={}ms ({:.0} MB/s)",
            avg_s,
            avg_p,
            avg_s + avg_p,
            file_data.len() as f64 / 1_048_576.0 / ((avg_s + avg_p) as f64 / 1000.0)
        );
    }

    /// Benchmark: USDC parse phases for audi.usdc.
    #[allow(unsafe_code)]
    #[test]
    #[ignore]
    fn bench_usdc_parse_audi() {
        let workspace =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../data/audi.usdc");
        if !workspace.exists() {
            eprintln!("audi.usdc not found, skipping");
            return;
        }
        let file_data = std::fs::read(&workspace).unwrap();
        eprintln!("File size: {:.1} MB", file_data.len() as f64 / 1_048_576.0);

        let t0 = std::time::Instant::now();
        let crate_file = CrateFile::open(&file_data, "audi.usdc").unwrap();
        let struct_ms = t0.elapsed().as_millis();

        let t1 = std::time::Instant::now();
        let layer = Layer::create_anonymous(Some(".usdc"));
        let layer_mut = unsafe { &mut *(Arc::as_ptr(&layer) as *mut Layer) };
        let fmt = UsdcFileFormat::new();
        fmt.populate_layer_from_crate(layer_mut, &crate_file, &file_data)
            .unwrap();
        let pop_ms = t1.elapsed().as_millis();

        eprintln!(
            "audi.usdc: struct={}ms populate={}ms total={}ms ({:.0} MB/s)",
            struct_ms,
            pop_ms,
            struct_ms + pop_ms,
            file_data.len() as f64 / 1_048_576.0 / ((struct_ms + pop_ms) as f64 / 1000.0)
        );
    }

    /// Diagnose why flo.usdc animated xformOp timeSamples return None.
    /// Run: cargo test -p usd-sdf diag_usdc_timesamples -- --nocapture --ignored
    #[test]
    #[ignore]
    fn diag_usdc_timesamples() {
        use crate::layer::Layer;
        use crate::{Path, init};

        init();

        let workspace =
            std::path::PathBuf::from("C:/projects/projects.rust.cg/usd-rs/data/flo.usdc");
        if !workspace.exists() {
            eprintln!("SKIP: flo.usdc not found at {:?}", workspace);
            return;
        }
        eprintln!("Opening: {:?}", workspace);

        // Open via the Layer API (same path Stage uses)
        let layer =
            Layer::find_or_open(workspace.to_str().unwrap()).expect("failed to open flo.usdc");

        // Check the animated translate attribute at layer level
        let trans_path =
            Path::from_string("/root/flo/noga_a/noga1/noga3_001.xformOp:translate").unwrap();
        let times = layer.list_time_samples_for_path(&trans_path);
        eprintln!("[Layer] xformOp:translate timeSamples = {}", times.len());
        if times.is_empty() {
            eprintln!("  => NO SAMPLES - timeSamples not stored in layer!");
        } else {
            for &t in times.iter().take(3) {
                match layer.query_time_sample(&trans_path, t) {
                    Some(v) => eprintln!("  t={} => type={:?}", t, v.type_name()),
                    None => eprintln!("  t={} => None", t),
                }
            }
        }

        // rotateXYZ
        let rot_path =
            Path::from_string("/root/flo/noga_a/noga1/noga3_001.xformOp:rotateXYZ").unwrap();
        let rot_times = layer.list_time_samples_for_path(&rot_path);
        eprintln!(
            "[Layer] xformOp:rotateXYZ timeSamples = {}",
            rot_times.len()
        );
        if let Some(&t0) = rot_times.first() {
            if let Some(v) = layer.query_time_sample(&rot_path, t0) {
                eprintln!("  first t={} => type={:?}", t0, v.type_name());
            }
        }

        // Raw CrateFile scan - find path entries containing the prim name
        let file_data = std::fs::read(&workspace).unwrap();
        let crate_file = CrateFile::open(&file_data, "flo.usdc").unwrap();
        eprintln!(
            "[Crate] total paths={} fields={}",
            crate_file.paths.len(),
            crate_file.fields.len()
        );
        let path_matches: Vec<_> = crate_file
            .paths
            .iter()
            .enumerate()
            .filter(|(_, p)| p.as_str().contains("noga3_001"))
            .take(5)
            .collect();
        eprintln!("[Crate] paths with 'noga3_001': {}", path_matches.len());
        for (i, p) in &path_matches {
            eprintln!("  [{}] {}", i, p.as_str());
        }

        // Find the field for xformOp:translate and inspect its ValueRep
        for (spec_idx, spec) in crate_file.specs.iter().enumerate() {
            let path_idx = spec.path_index.0.value as usize;
            if path_idx >= crate_file.paths.len() {
                continue;
            }
            let path_str = crate_file.paths[path_idx].as_str();
            if !path_str.contains("noga3_001.xformOp:translate") {
                continue;
            }

            eprintln!("[Crate] Found spec[{}] for {}", spec_idx, path_str);
            let fsi = spec.field_set_index.0.value as usize;
            let mut fi = fsi;
            while fi < crate_file.field_sets.len() {
                let fld_idx = crate_file.field_sets[fi];
                if !fld_idx.0.is_valid() {
                    break;
                }
                if let Some(field) = crate_file.get_field(fld_idx) {
                    if let Some(tok) = crate_file.get_token(field.token_index) {
                        let rep = &field.value_rep;
                        eprintln!(
                            "  field '{}': type={:?} inlined={} array={} payload=0x{:x}",
                            tok.as_str(),
                            rep.get_type(),
                            rep.is_inlined(),
                            rep.is_array(),
                            rep.get_payload()
                        );
                        if tok == "timeSamples" {
                            let offset = rep.get_payload() as usize;
                            eprintln!(
                                "  TimeSamples offset=0x{:x} file_len=0x{:x}",
                                offset,
                                file_data.len()
                            );
                            if offset + 8 <= file_data.len() {
                                // Read jump1 raw bytes
                                let raw8 = &file_data[offset..offset + 8];
                                let jump1_raw = i64::from_le_bytes([
                                    raw8[0], raw8[1], raw8[2], raw8[3], raw8[4], raw8[5], raw8[6],
                                    raw8[7],
                                ]);
                                eprintln!("  jump1 raw bytes = {:02x?}", raw8);
                                eprintln!("  jump1 as i64 = {} (0x{:x})", jump1_raw, jump1_raw);
                                let times_rep_offset =
                                    (offset as i64).wrapping_add(jump1_raw) as usize;
                                eprintln!("  times_rep_offset = 0x{:x}", times_rep_offset);
                                if times_rep_offset + 8 <= file_data.len() {
                                    let tr = &file_data[times_rep_offset..times_rep_offset + 8];
                                    eprintln!("  timesRep bytes = {:02x?}", tr);
                                    // jump2
                                    let jump2_pos = times_rep_offset + 8;
                                    if jump2_pos + 8 <= file_data.len() {
                                        let j2b = &file_data[jump2_pos..jump2_pos + 8];
                                        let jump2 = i64::from_le_bytes([
                                            j2b[0], j2b[1], j2b[2], j2b[3], j2b[4], j2b[5], j2b[6],
                                            j2b[7],
                                        ]);
                                        eprintln!("  jump2 = {} (0x{:x})", jump2, jump2);
                                        let num_values_pos =
                                            (jump2_pos as i64).wrapping_add(jump2) as usize;
                                        eprintln!("  num_values_pos = 0x{:x}", num_values_pos);
                                        if num_values_pos + 8 <= file_data.len() {
                                            let nv = u64::from_le_bytes([
                                                file_data[num_values_pos],
                                                file_data[num_values_pos + 1],
                                                file_data[num_values_pos + 2],
                                                file_data[num_values_pos + 3],
                                                file_data[num_values_pos + 4],
                                                file_data[num_values_pos + 5],
                                                file_data[num_values_pos + 6],
                                                file_data[num_values_pos + 7],
                                            ]);
                                            eprintln!("  num_values = {}", nv);
                                        } else {
                                            eprintln!("  num_values_pos OUT OF BOUNDS");
                                        }
                                    }
                                } else {
                                    eprintln!("  times_rep_offset OUT OF BOUNDS!");
                                }
                            }
                            let fmt = UsdcFileFormat::new();
                            let v = fmt.unpack_value(rep, &crate_file, &file_data);
                            match v {
                                Some(ref val) => {
                                    eprintln!("  unpack_value => type={:?}", val.type_name());
                                    if let Some(ts) = val.downcast::<CrateTimeSamples>() {
                                        eprintln!(
                                            "  CrateTimeSamples: times={} values={}",
                                            ts.times.len(),
                                            ts.values.len()
                                        );
                                        for i in 0..ts.times.len().min(3) {
                                            let vt = ts
                                                .values
                                                .get(i)
                                                .map(|v| format!("{:?}", v.type_name()))
                                                .unwrap_or_else(|| "MISSING".to_string());
                                            eprintln!(
                                                "    [{}] t={} val_type={}",
                                                i, ts.times[i], vt
                                            );
                                        }
                                    } else {
                                        eprintln!("  NOT a CrateTimeSamples!");
                                    }
                                }
                                None => eprintln!("  unpack_value => None!"),
                            }
                        }
                    }
                }
                fi += 1;
            }
        }
    }

    /// Comprehensive round-trip test: create a hierarchy with xformOps + time-sampled
    /// animation in USDA, write as USDC, read back, verify ALL values match.
    /// This tests the critical Array<Token> vs Vec<Token> bridge that caused
    /// "exploded" USDC transforms.
    #[test]
    fn test_usdc_roundtrip_hierarchy_xform_animation() {
        use usd_gf::vec3::Vec3d;

        // ===== Step 1: Build a layer with known hierarchy + xformOps + time samples =====
        let layer = crate::Layer::create_anonymous(Some("test_xform_roundtrip"));

        // Create hierarchy: /World/Parent/Child
        let world_path = crate::Path::from_string("/World").unwrap();
        let parent_path = crate::Path::from_string("/World/Parent").unwrap();
        let child_path = crate::Path::from_string("/World/Parent/Child").unwrap();

        layer.create_prim_spec(&world_path, crate::Specifier::Def, "Xform");
        layer.create_prim_spec(&parent_path, crate::Specifier::Def, "Xform");
        layer.create_prim_spec(&child_path, crate::Specifier::Def, "Mesh");

        // Set xformOpOrder on Parent (Token array — this is the critical field!)
        let xform_op_order_path = parent_path.append_property("xformOpOrder").unwrap();
        layer.create_spec(&xform_op_order_path, crate::SpecType::Attribute);
        let order_tokens = vec![
            usd_tf::Token::new("xformOp:translate"),
            usd_tf::Token::new("xformOp:rotateXYZ"),
            usd_tf::Token::new("xformOp:scale"),
        ];
        layer.set_field(
            &xform_op_order_path,
            &Token::new("default"),
            usd_vt::Value::new(order_tokens.clone()),
        );
        layer.set_field(
            &xform_op_order_path,
            &Token::new("typeName"),
            usd_vt::Value::new(Token::new("token[]")),
        );
        layer.set_field(
            &xform_op_order_path,
            &Token::new("variability"),
            usd_vt::Value::new(crate::Variability::Uniform),
        );

        // Set xformOp:translate with time samples on Parent
        let translate_path = parent_path.append_property("xformOp:translate").unwrap();
        layer.create_spec(&translate_path, crate::SpecType::Attribute);
        layer.set_field(
            &translate_path,
            &Token::new("typeName"),
            usd_vt::Value::new(Token::new("double3")),
        );
        // Time samples: t=1.0 → (10, 20, 30), t=24.0 → (100, 200, 300)
        let tr_t1 = Vec3d::new(10.0, 20.0, 30.0);
        let tr_t24 = Vec3d::new(100.0, 200.0, 300.0);
        layer.set_time_sample(&translate_path, 1.0, usd_vt::Value::from_no_hash(tr_t1));
        layer.set_time_sample(&translate_path, 24.0, usd_vt::Value::from_no_hash(tr_t24));

        // Set xformOp:rotateXYZ with default value
        let rotate_path = parent_path.append_property("xformOp:rotateXYZ").unwrap();
        layer.create_spec(&rotate_path, crate::SpecType::Attribute);
        layer.set_field(
            &rotate_path,
            &Token::new("typeName"),
            usd_vt::Value::new(Token::new("double3")),
        );
        let rot_val = Vec3d::new(0.0, 45.0, 0.0);
        layer.set_field(
            &rotate_path,
            &Token::new("default"),
            usd_vt::Value::from_no_hash(rot_val),
        );

        // Set xformOp:scale with default value
        let scale_path = parent_path.append_property("xformOp:scale").unwrap();
        layer.create_spec(&scale_path, crate::SpecType::Attribute);
        layer.set_field(
            &scale_path,
            &Token::new("typeName"),
            usd_vt::Value::new(Token::new("double3")),
        );
        let scale_val = Vec3d::new(2.0, 2.0, 2.0);
        layer.set_field(
            &scale_path,
            &Token::new("default"),
            usd_vt::Value::from_no_hash(scale_val),
        );

        // ===== Step 2: Write as USDC =====
        let tmp_dir = std::env::temp_dir();
        let usdc_path = tmp_dir.join("test_xform_roundtrip.usdc");
        let format = UsdcFileFormat::new();
        format
            .write_to_file(
                &layer,
                usdc_path.to_str().unwrap(),
                None,
                &crate::file_format::FileFormatArguments::new(),
            )
            .expect("Failed to write USDC file");

        // ===== Step 3: Read back from USDC =====
        crate::init();
        let layer2 = crate::Layer::find_or_open(usdc_path.to_str().unwrap())
            .unwrap_or_else(|e| panic!("Failed to open USDC file: {:?}: {}", usdc_path, e));

        // ===== Step 4: Verify hierarchy =====
        // Check root prims
        let root_prims = layer2.get_root_prims();
        assert_eq!(
            root_prims.len(),
            1,
            "Expected 1 root prim (/World), got {}",
            root_prims.len()
        );
        assert_eq!(root_prims[0].path(), world_path);

        // Check primChildren of /World — must find Parent
        let world_children_token = Token::new("primChildren");
        let world_children = layer2.get_field(&world_path, &world_children_token);
        assert!(
            world_children.is_some(),
            "/World primChildren field missing"
        );
        let world_child_names = world_children
            .unwrap()
            .as_vec_clone::<Token>()
            .expect("/World primChildren: as_vec_clone::<Token> failed (Array<Token> vs Vec<Token> bridge broken)");
        assert_eq!(world_child_names.len(), 1);
        assert_eq!(world_child_names[0].as_str(), "Parent");

        // Check primChildren of /World/Parent — must find Child
        let parent_children = layer2.get_field(&parent_path, &world_children_token);
        assert!(
            parent_children.is_some(),
            "/World/Parent primChildren missing"
        );
        let parent_child_names = parent_children
            .unwrap()
            .as_vec_clone::<Token>()
            .expect("/World/Parent primChildren: as_vec_clone failed");
        assert_eq!(parent_child_names.len(), 1);
        assert_eq!(parent_child_names[0].as_str(), "Child");

        // ===== Step 5: Verify xformOpOrder (THE CRITICAL TEST) =====
        let xform_op_order_val = layer2
            .get_field(&xform_op_order_path, &Token::new("default"))
            .expect("xformOpOrder default field missing after USDC round-trip");

        // This MUST work with as_vec_clone — it's Array<Token> from USDC
        let read_order = xform_op_order_val
            .as_vec_clone::<Token>()
            .expect("CRITICAL: xformOpOrder as_vec_clone::<Token> failed! This causes exploded USDC transforms.");
        assert_eq!(read_order.len(), 3, "xformOpOrder should have 3 entries");
        assert_eq!(read_order[0].as_str(), "xformOp:translate");
        assert_eq!(read_order[1].as_str(), "xformOp:rotateXYZ");
        assert_eq!(read_order[2].as_str(), "xformOp:scale");

        // ===== Step 6: Verify time-sampled translate values =====
        let ts_t1 = layer2.query_time_sample(&translate_path, 1.0);
        assert!(
            ts_t1.is_some(),
            "Time sample at t=1.0 missing for xformOp:translate"
        );
        let ts_t1_vec = ts_t1.unwrap();
        if let Some(&v) = ts_t1_vec.downcast::<Vec3d>() {
            assert!(
                (v.x - 10.0).abs() < 1e-9 && (v.y - 20.0).abs() < 1e-9 && (v.z - 30.0).abs() < 1e-9,
                "translate at t=1.0: expected (10,20,30), got ({},{},{})",
                v.x,
                v.y,
                v.z
            );
        } else {
            panic!(
                "translate at t=1.0: expected Vec3d, got type {:?}",
                ts_t1_vec.type_name()
            );
        }

        let ts_t24 = layer2.query_time_sample(&translate_path, 24.0);
        assert!(ts_t24.is_some(), "Time sample at t=24.0 missing");
        let ts_t24_vec = ts_t24.unwrap();
        if let Some(&v) = ts_t24_vec.downcast::<Vec3d>() {
            assert!(
                (v.x - 100.0).abs() < 1e-9
                    && (v.y - 200.0).abs() < 1e-9
                    && (v.z - 300.0).abs() < 1e-9,
                "translate at t=24.0: expected (100,200,300), got ({},{},{})",
                v.x,
                v.y,
                v.z
            );
        } else {
            panic!(
                "translate at t=24.0: expected Vec3d, got type {:?}",
                ts_t24_vec.type_name()
            );
        }

        // ===== Step 7: Verify default values for rotate/scale =====
        let rot_read = layer2
            .get_field(&rotate_path, &Token::new("default"))
            .expect("rotateXYZ default missing");
        if let Some(&v) = rot_read.downcast::<Vec3d>() {
            assert!(
                (v.x - 0.0).abs() < 1e-9 && (v.y - 45.0).abs() < 1e-9 && (v.z - 0.0).abs() < 1e-9,
                "rotateXYZ default: expected (0,45,0), got ({},{},{})",
                v.x,
                v.y,
                v.z
            );
        } else {
            panic!(
                "rotateXYZ default: expected Vec3d, got {:?}",
                rot_read.type_name()
            );
        }

        let scale_read = layer2
            .get_field(&scale_path, &Token::new("default"))
            .expect("scale default missing");
        if let Some(&v) = scale_read.downcast::<Vec3d>() {
            assert!(
                (v.x - 2.0).abs() < 1e-9 && (v.y - 2.0).abs() < 1e-9 && (v.z - 2.0).abs() < 1e-9,
                "scale default: expected (2,2,2), got ({},{},{})",
                v.x,
                v.y,
                v.z
            );
        } else {
            panic!(
                "scale default: expected Vec3d, got {:?}",
                scale_read.type_name()
            );
        }

        // Cleanup
        let _ = std::fs::remove_file(&usdc_path);

        eprintln!(
            "USDC round-trip test PASSED: hierarchy + xformOpOrder + time samples + defaults all correct."
        );
    }
}
