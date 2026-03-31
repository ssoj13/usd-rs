//! USDC binary format reader — CrateFile struct and all read/decode methods.
//!
//! CrateFile handles reading the structural sections of a USDC crate file:
//! TOKENS, STRINGS, FIELDS, FIELDSETS, PATHS, SPECS.

use usd_tf::Token;

use crate::file_format::FileFormatError;
use crate::path::Path;
use crate::types::SpecType;

use super::types::{
    BOOTSTRAP_SIZE, Bootstrap, CrateSpec, Field, FieldIndex, FieldSetIndex, Index,
    MIN_READ_VERSION, PathIndex, SOFTWARE_VERSION, StringIndex, TableOfContents, TokenIndex,
    USDC_MAGIC, ValueRep, section_names,
};

// ============================================================================
// CrateFile - Reader/Writer for USDC binary format
// ============================================================================

/// Crate file reader/writer for USDC binary format.
///
/// This handles reading and writing the structural sections of a crate file:
/// - TOKENS: string tokens (TfToken equivalents)
/// - STRINGS: string indices -> token indices
/// - FIELDS: field definitions (token index + value rep)
/// - FIELDSETS: groups of field indices
/// - PATHS: path hierarchy (compressed in version >= 0.4.0)
/// - SPECS: spec definitions (path index + fieldset index + type)
pub struct CrateFile {
    /// File version (major, minor, patch)
    pub version: (u8, u8, u8),
    /// Bootstrap header
    pub bootstrap: Bootstrap,
    /// Table of contents
    pub toc: TableOfContents,
    /// Token table
    pub tokens: Vec<Token>,
    /// String table (indices into tokens)
    pub strings: Vec<TokenIndex>,
    /// Field table
    pub fields: Vec<Field>,
    /// Field set table (field indices, invalid-terminated groups)
    pub field_sets: Vec<FieldIndex>,
    /// Path table
    pub paths: Vec<Path>,
    /// Spec table
    pub specs: Vec<CrateSpec>,
    /// Source file path
    pub asset_path: String,
    /// Whether data is detached from file
    pub detached: bool,
}

impl CrateFile {
    /// Creates a new empty crate file.
    #[must_use]
    pub fn new() -> Self {
        Self {
            version: SOFTWARE_VERSION,
            bootstrap: Bootstrap::default(),
            toc: TableOfContents::new(),
            tokens: Vec::new(),
            strings: Vec::new(),
            fields: Vec::new(),
            field_sets: Vec::new(),
            paths: Vec::new(),
            specs: Vec::new(),
            asset_path: String::new(),
            detached: false,
        }
    }

    /// Creates a detached crate file.
    #[must_use]
    pub fn new_detached() -> Self {
        let mut cf = Self::new();
        cf.detached = true;
        cf
    }

    /// Returns whether this crate file can read the given data.
    #[must_use]
    pub fn can_read(data: &[u8]) -> bool {
        data.len() >= BOOTSTRAP_SIZE && &data[0..8] == USDC_MAGIC
    }

    /// Opens a crate file from bytes.
    pub fn open(data: &[u8], asset_path: &str) -> Result<Self, FileFormatError> {
        usd_trace::trace_scope!("usdc_open");
        if !Self::can_read(data) {
            return Err(FileFormatError::corrupt_file(
                asset_path,
                "Not a valid crate file (missing PXR-USDC magic)",
            ));
        }

        let bootstrap = Bootstrap::from_bytes(data)?;
        let version = bootstrap.version_tuple();

        // Validate version
        if version < MIN_READ_VERSION {
            return Err(FileFormatError::version_mismatch(
                format!(
                    "{}.{}.{}",
                    MIN_READ_VERSION.0, MIN_READ_VERSION.1, MIN_READ_VERSION.2
                ),
                format!("{}.{}.{}", version.0, version.1, version.2),
            ));
        }

        let mut crate_file = Self {
            version,
            bootstrap,
            toc: TableOfContents::new(),
            tokens: Vec::new(),
            strings: Vec::new(),
            fields: Vec::new(),
            field_sets: Vec::new(),
            paths: Vec::new(),
            specs: Vec::new(),
            asset_path: asset_path.to_string(),
            detached: false,
        };

        // Read table of contents
        let toc_offset = crate_file.bootstrap.toc_offset as usize;
        if toc_offset >= data.len() {
            return Err(FileFormatError::corrupt_file(
                asset_path,
                "ToC offset beyond file end",
            ));
        }

        // Read number of sections (u64 at toc_offset)
        if toc_offset + 8 > data.len() {
            return Err(FileFormatError::corrupt_file(
                asset_path,
                "Cannot read section count",
            ));
        }
        let num_sections = u64::from_le_bytes([
            data[toc_offset],
            data[toc_offset + 1],
            data[toc_offset + 2],
            data[toc_offset + 3],
            data[toc_offset + 4],
            data[toc_offset + 5],
            data[toc_offset + 6],
            data[toc_offset + 7],
        ]) as usize;

        // Read sections
        let sections_start = toc_offset + 8;
        crate_file.toc = TableOfContents::from_bytes(&data[sections_start..], num_sections)?;

        // Read structural sections
        crate_file.read_tokens(data)?;
        crate_file.read_strings(data)?;
        crate_file.read_fields(data)?;
        crate_file.read_field_sets(data)?;
        crate_file.read_paths(data)?;
        crate_file.read_specs(data)?;

        Ok(crate_file)
    }

    /// Reads token section.
    fn read_tokens(&mut self, data: &[u8]) -> Result<(), FileFormatError> {
        usd_trace::trace_scope!("usdc_read_tokens");
        let Some(section) = self.toc.get_section(section_names::TOKENS) else {
            return Ok(());
        };

        let start = section.start as usize;
        if start + 8 > data.len() {
            return Err(FileFormatError::corrupt_file(
                &self.asset_path,
                "Tokens section truncated",
            ));
        }

        let mut offset = start;

        // Read number of tokens
        let num_tokens = u64::from_le_bytes([
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

        // Get token bytes based on version
        let token_bytes: Vec<u8>;
        if self.version < (0, 4, 0) {
            // Uncompressed tokens: num_bytes followed by null-terminated strings
            let num_bytes = u64::from_le_bytes([
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

            if offset + num_bytes > data.len() {
                return Err(FileFormatError::corrupt_file(
                    &self.asset_path,
                    "Token data truncated",
                ));
            }
            token_bytes = data[offset..offset + num_bytes].to_vec();
        } else {
            // Compressed tokens (version >= 0.4.0): uncompressed_size, compressed_size, lz4 data
            let uncompressed_size = u64::from_le_bytes([
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

            let compressed_size = u64::from_le_bytes([
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

            if offset + compressed_size > data.len() {
                return Err(FileFormatError::corrupt_file(
                    &self.asset_path,
                    "Compressed token data truncated",
                ));
            }

            // Decompress using LZ4
            let compressed = &data[offset..offset + compressed_size];
            token_bytes = usd_tf::fast_compression::FastCompression::decompress(
                compressed,
                uncompressed_size,
            )
            .map_err(|e| {
                FileFormatError::corrupt_file(
                    &self.asset_path,
                    format!("LZ4 decompression failed: {:?}", e),
                )
            })?;
        }

        // Parse null-terminated strings into tokens
        self.tokens.clear();
        self.tokens.reserve(num_tokens);

        let mut p = 0;
        for _ in 0..num_tokens {
            // Find null terminator
            let end = token_bytes[p..]
                .iter()
                .position(|&b| b == 0)
                .map(|i| p + i)
                .unwrap_or(token_bytes.len());

            let s = std::str::from_utf8(&token_bytes[p..end]).unwrap_or("");
            self.tokens.push(Token::new(s));
            p = end + 1;

            if p > token_bytes.len() {
                break;
            }
        }

        Ok(())
    }

    /// Reads string section.
    fn read_strings(&mut self, data: &[u8]) -> Result<(), FileFormatError> {
        usd_trace::trace_scope!("usdc_read_strings");
        let Some(section) = self.toc.get_section(section_names::STRINGS) else {
            return Ok(());
        };

        let start = section.start as usize;
        let size = section.size as usize;

        if start + size > data.len() {
            return Err(FileFormatError::corrupt_file(
                &self.asset_path,
                "Strings section truncated",
            ));
        }

        // Strings section is array of u32 token indices
        // First read count
        if size < 8 {
            return Ok(()); // Empty section
        }

        let count = u64::from_le_bytes([
            data[start],
            data[start + 1],
            data[start + 2],
            data[start + 3],
            data[start + 4],
            data[start + 5],
            data[start + 6],
            data[start + 7],
        ]) as usize;

        self.strings.clear();
        self.strings.reserve(count);

        let mut offset = start + 8;
        for _ in 0..count {
            if offset + 4 > data.len() {
                break;
            }
            let token_idx = u32::from_le_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ]);
            self.strings.push(TokenIndex::new(token_idx));
            offset += 4;
        }

        Ok(())
    }

    /// Helper: reads a u64 from data at offset.
    fn read_u64_at(data: &[u8], offset: usize) -> usize {
        u64::from_le_bytes([
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
            data[offset + 4],
            data[offset + 5],
            data[offset + 6],
            data[offset + 7],
        ]) as usize
    }

    /// Helper: reads compressed u32 integers (_CompressedIntsReader::Read equivalent).
    /// Returns (decompressed values, bytes consumed from data).
    fn read_compressed_u32(
        data: &[u8],
        offset: usize,
        num_ints: usize,
        asset_path: &str,
        context: &str,
    ) -> Result<(Vec<u32>, usize), FileFormatError> {
        use crate::integer_coding::IntegerCompression;

        let compressed_size = Self::read_u64_at(data, offset);
        let data_start = offset + 8;

        let values = IntegerCompression::decompress_u32(
            &data[data_start..data_start + compressed_size],
            num_ints,
        )
        .map_err(|e| {
            FileFormatError::corrupt_file(
                asset_path,
                format!("Failed to decompress {}: {}", context, e),
            )
        })?;

        Ok((values, 8 + compressed_size))
    }

    /// Reads fields section.
    fn read_fields(&mut self, data: &[u8]) -> Result<(), FileFormatError> {
        usd_trace::trace_scope!("usdc_read_fields");
        let Some(section) = self.toc.get_section(section_names::FIELDS) else {
            return Ok(());
        };

        let start = section.start as usize;
        let size = section.size as usize;

        if start + size > data.len() {
            return Err(FileFormatError::corrupt_file(
                &self.asset_path,
                "Fields section truncated",
            ));
        }

        if size < 8 {
            return Ok(());
        }

        let count = Self::read_u64_at(data, start);
        let mut offset = start + 8;

        if self.version < (0, 4, 0) {
            // Uncompressed fields: raw struct data
            self.fields.clear();
            self.fields.reserve(count);

            for _ in 0..count {
                if offset + Field::SIZE > data.len() {
                    break;
                }
                let field = Field::from_bytes(&data[offset..])?;
                self.fields.push(field);
                offset += Field::SIZE;
            }
        } else {
            // Compressed fields (v0.4.0+):
            // 1. Compressed tokenIndexes (u32 array via _ReadCompressedInts)
            // 2. repsSize (u64) + compressed valueReps (raw TfFastCompression of u64 array)

            let (token_indexes, consumed) = Self::read_compressed_u32(
                data,
                offset,
                count,
                &self.asset_path,
                "field token indexes",
            )?;
            offset += consumed;

            // Read compressed value reps
            let reps_size = Self::read_u64_at(data, offset);
            offset += 8;

            let compressed_reps = &data[offset..offset + reps_size];
            let max_output = count * 8; // u64 per field
            let decompressed_reps =
                usd_tf::fast_compression::FastCompression::decompress(compressed_reps, max_output)
                    .map_err(|e| {
                        FileFormatError::corrupt_file(
                            &self.asset_path,
                            format!("Failed to decompress field value reps: {:?}", e),
                        )
                    })?;

            self.fields.clear();
            self.fields.reserve(count);

            for i in 0..count {
                let token_index = TokenIndex(Index::new(token_indexes[i]));
                let value_rep = if i * 8 + 8 <= decompressed_reps.len() {
                    let rep_data = u64::from_le_bytes([
                        decompressed_reps[i * 8],
                        decompressed_reps[i * 8 + 1],
                        decompressed_reps[i * 8 + 2],
                        decompressed_reps[i * 8 + 3],
                        decompressed_reps[i * 8 + 4],
                        decompressed_reps[i * 8 + 5],
                        decompressed_reps[i * 8 + 6],
                        decompressed_reps[i * 8 + 7],
                    ]);
                    ValueRep { data: rep_data }
                } else {
                    ValueRep { data: 0 }
                };
                self.fields.push(Field {
                    padding: 0,
                    token_index,
                    value_rep,
                });
            }
        }

        Ok(())
    }

    /// Reads field sets section.
    fn read_field_sets(&mut self, data: &[u8]) -> Result<(), FileFormatError> {
        let Some(section) = self.toc.get_section(section_names::FIELDSETS) else {
            return Ok(());
        };

        let start = section.start as usize;
        let size = section.size as usize;

        if start + size > data.len() {
            return Err(FileFormatError::corrupt_file(
                &self.asset_path,
                "FieldSets section truncated",
            ));
        }

        if size < 8 {
            return Ok(());
        }

        let count = Self::read_u64_at(data, start);
        let offset = start + 8;

        if self.version < (0, 4, 0) {
            // Uncompressed field sets: raw u32 array
            self.field_sets.clear();
            self.field_sets.reserve(count);

            let mut off = offset;
            for _ in 0..count {
                if off + 4 > data.len() {
                    break;
                }
                let idx =
                    u32::from_le_bytes([data[off], data[off + 1], data[off + 2], data[off + 3]]);
                self.field_sets.push(FieldIndex::new(idx));
                off += 4;
            }
        } else {
            // Compressed field sets (v0.4.0+): compressed u32 array
            let (values, _consumed) = Self::read_compressed_u32(
                data,
                offset,
                count,
                &self.asset_path,
                "field set indexes",
            )?;

            self.field_sets.clear();
            self.field_sets.reserve(count);
            for v in values {
                self.field_sets.push(FieldIndex::new(v));
            }
        }

        // Ensure termination
        if !self.field_sets.is_empty() && self.field_sets.last().map_or(false, |f| f.0.is_valid()) {
            if let Some(last) = self.field_sets.last_mut() {
                *last = FieldIndex::invalid();
            }
        }

        Ok(())
    }

    /// Reads paths section.
    fn read_paths(&mut self, data: &[u8]) -> Result<(), FileFormatError> {
        usd_trace::trace_scope!("usdc_read_paths");
        let Some(section) = self.toc.get_section(section_names::PATHS) else {
            return Ok(());
        };

        let start = section.start as usize;
        if start >= data.len() {
            return Err(FileFormatError::corrupt_file(
                &self.asset_path,
                "Paths section truncated",
            ));
        }

        // Read number of paths
        let num_paths = u64::from_le_bytes([
            data[start],
            data[start + 1],
            data[start + 2],
            data[start + 3],
            data[start + 4],
            data[start + 5],
            data[start + 6],
            data[start + 7],
        ]) as usize;

        // Pre-allocate paths vector
        self.paths.clear();
        self.paths.resize(num_paths, Path::empty());

        if num_paths == 0 {
            return Ok(());
        }

        if self.version >= (0, 4, 0) {
            // Compressed paths (version >= 0.4.0)
            self.read_compressed_paths(data, start + 8, num_paths)?;
        } else {
            // Uncompressed paths - tree structure
            self.read_uncompressed_paths(data, start + 8, num_paths)?;
        }

        Ok(())
    }

    /// Reads compressed paths (version >= 0.4.0).
    ///
    /// C++ layout: `_ReadPaths` reads total_paths at section start (used to pre-allocate
    /// `_paths[]`), then `_ReadCompressedPaths` reads `numPaths` (non-empty path count),
    /// followed by three compressed integer arrays (pathIndexes, elementTokenIndexes, jumps),
    /// each prefixed with a u64 compressedSize.
    ///
    /// The `_paths_total` parameter is the total pre-allocated paths count (including the
    /// empty-path slot at index 0).  The actual compressed arrays have `numPaths` elements
    /// (non-empty paths only), which is read from the file here.
    fn read_compressed_paths(
        &mut self,
        data: &[u8],
        offset: usize,
        _paths_total: usize,
    ) -> Result<(), FileFormatError> {
        use crate::integer_coding::IntegerCompression;

        let mut offset = offset;
        let section_end = data.len();

        // Helper to read u64 at offset
        let read_u64 = |off: usize| -> usize {
            if off + 8 > section_end {
                return 0;
            }
            u64::from_le_bytes([
                data[off],
                data[off + 1],
                data[off + 2],
                data[off + 3],
                data[off + 4],
                data[off + 5],
                data[off + 6],
                data[off + 7],
            ]) as usize
        };

        // C++ _ReadCompressedPaths reads the non-empty path count from the file.
        // This is the number of elements in each compressed array (excludes empty path).
        let num_encoded = read_u64(offset);
        offset += 8;

        if num_encoded == 0 {
            return Ok(());
        }

        // Read pathIndexes (compressed u32 array)
        // Each compressed array: [u64 compressedSize] [compressedSize bytes of data]
        let pi_compressed_size = read_u64(offset);
        offset += 8;

        let path_indexes = IntegerCompression::decompress_u32(
            &data[offset..offset + pi_compressed_size],
            num_encoded,
        )
        .map_err(|e| {
            FileFormatError::corrupt_file(
                &self.asset_path,
                format!("Failed to decompress path indexes: {}", e),
            )
        })?;
        offset += pi_compressed_size;

        // Read elementTokenIndexes (compressed i32 array)
        let eti_compressed_size = read_u64(offset);
        offset += 8;

        let element_token_indexes = IntegerCompression::decompress_i32(
            &data[offset..offset + eti_compressed_size],
            num_encoded,
        )
        .map_err(|e| {
            FileFormatError::corrupt_file(
                &self.asset_path,
                format!("Failed to decompress element token indexes: {}", e),
            )
        })?;
        offset += eti_compressed_size;

        // Read jumps (compressed i32 array)
        let jumps_compressed_size = read_u64(offset);
        offset += 8;

        let jumps = IntegerCompression::decompress_i32(
            &data[offset..offset + jumps_compressed_size],
            num_encoded,
        )
        .map_err(|e| {
            FileFormatError::corrupt_file(
                &self.asset_path,
                format!("Failed to decompress jumps: {}", e),
            )
        })?;

        // Build decompressed paths
        self.build_decompressed_paths(
            &path_indexes,
            &element_token_indexes,
            &jumps,
            0,
            Path::empty(),
        );

        Ok(())
    }

    /// Builds decompressed paths from compressed data.
    fn build_decompressed_paths(
        &mut self,
        path_indexes: &[u32],
        element_token_indexes: &[i32],
        jumps: &[i32],
        mut cur_index: usize,
        mut parent_path: Path,
    ) {
        let mut has_child;
        let mut has_sibling;

        loop {
            if cur_index >= path_indexes.len() {
                break;
            }

            let this_index = cur_index;
            cur_index += 1;

            let path_idx = path_indexes[this_index] as usize;
            if path_idx >= self.paths.len() {
                break;
            }

            if parent_path.is_empty() {
                // Root path
                parent_path = Path::absolute_root();
                self.paths[path_idx] = parent_path.clone();
            } else {
                // Build path from parent + element token
                let token_index = element_token_indexes[this_index];
                let is_property = token_index < 0;
                let abs_token_index = token_index.unsigned_abs() as usize;

                if abs_token_index < self.tokens.len() {
                    let elem_token = self.tokens[abs_token_index].as_str();
                    let new_path = if is_property {
                        parent_path.append_property(elem_token)
                    } else {
                        // C++ uses AppendElementToken which handles variant
                        // paths (tokens starting with '{') by parsing
                        // {variantSet=selection} and calling
                        // AppendVariantSelection.
                        parent_path.append_element_token(&Token::new(elem_token))
                    };
                    if let Some(path) = new_path {
                        self.paths[path_idx] = path;
                    }
                }
            }

            // Determine tree structure from jumps
            let jump = jumps[this_index];
            has_child = jump > 0 || jump == -1;
            has_sibling = jump >= 0;

            if has_child {
                if has_sibling {
                    // Process sibling subtree recursively
                    let sibling_index = this_index.wrapping_add(jump as usize);
                    self.build_decompressed_paths(
                        path_indexes,
                        element_token_indexes,
                        jumps,
                        sibling_index,
                        parent_path.clone(),
                    );
                }
                // Continue with child - update parent path
                if path_idx < self.paths.len() {
                    parent_path = self.paths[path_idx].clone();
                }
            }

            if !has_child && !has_sibling {
                break;
            }
        }
    }

    /// Reads uncompressed paths (version < 0.4.0).
    fn read_uncompressed_paths(
        &mut self,
        data: &[u8],
        offset: usize,
        _num_paths: usize,
    ) -> Result<(), FileFormatError> {
        // Path item header structure depends on version
        // For version < 0.0.1: 16-byte header with padding (gcc bug workaround)
        // For version 0.0.1-0.3.x: 12-byte header
        //
        // Bits: HasChild (1), HasSibling (2), IsPrimPropertyPath (4)

        if self.version < (0, 1, 0) {
            self.read_uncompressed_paths_v001(data, offset)
        } else {
            self.read_uncompressed_paths_v010(data, offset)
        }
    }

    /// Read paths in version 0.0.1 format (16-byte header with padding).
    fn read_uncompressed_paths_v001(
        &mut self,
        data: &[u8],
        start_offset: usize,
    ) -> Result<(), FileFormatError> {
        const HEADER_SIZE: usize = 16;
        const HAS_CHILD: u8 = 1;
        const HAS_SIBLING: u8 = 2;
        const IS_PRIM_PROPERTY_PATH: u8 = 4;

        // Stack for iterative tree traversal: (offset, parent_path)
        let mut stack: Vec<(usize, Path)> = vec![(start_offset, Path::empty())];

        while let Some((mut offset, mut parent_path)) = stack.pop() {
            loop {
                if offset + HEADER_SIZE > data.len() {
                    break;
                }

                // _PathItemHeader_0_0_1: 4 bytes padding + PathIndex + TokenIndex + bits
                // let _padding = u32 at offset
                let index = u32::from_le_bytes([
                    data[offset + 4],
                    data[offset + 5],
                    data[offset + 6],
                    data[offset + 7],
                ]) as usize;

                let element_token_index = i32::from_le_bytes([
                    data[offset + 8],
                    data[offset + 9],
                    data[offset + 10],
                    data[offset + 11],
                ]);

                let bits = data[offset + 12];
                offset += HEADER_SIZE;

                if index >= self.paths.len() {
                    break;
                }

                // Build path
                if parent_path.is_empty() {
                    parent_path = Path::absolute_root();
                    self.paths[index] = parent_path.clone();
                } else {
                    let abs_token_idx = element_token_index.unsigned_abs() as usize;
                    if abs_token_idx < self.tokens.len() {
                        let elem_token = self.tokens[abs_token_idx].as_str();
                        let is_property = (bits & IS_PRIM_PROPERTY_PATH) != 0;
                        let new_path = if is_property {
                            parent_path.append_property(elem_token)
                        } else {
                            parent_path.append_element_token(&Token::new(elem_token))
                        };
                        if let Some(path) = new_path {
                            self.paths[index] = path;
                        }
                    }
                }

                let has_child = (bits & HAS_CHILD) != 0;
                let has_sibling = (bits & HAS_SIBLING) != 0;

                if has_child {
                    if has_sibling {
                        // Read sibling offset and push to stack for later
                        if offset + 8 <= data.len() {
                            let sibling_offset = i64::from_le_bytes([
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
                            stack.push((sibling_offset, parent_path.clone()));
                        }
                    }
                    // Continue with child - update parent path
                    parent_path = self.paths[index].clone();
                } else if !has_sibling {
                    break;
                }
                // If only has_sibling, continue with next item (parent unchanged)
            }
        }

        Ok(())
    }

    /// Read paths in version 0.1.0+ format (12-byte header).
    fn read_uncompressed_paths_v010(
        &mut self,
        data: &[u8],
        start_offset: usize,
    ) -> Result<(), FileFormatError> {
        const HEADER_SIZE: usize = 12;
        const HAS_CHILD: u8 = 1;
        const HAS_SIBLING: u8 = 2;
        const IS_PRIM_PROPERTY_PATH: u8 = 4;

        // Stack for iterative tree traversal: (offset, parent_path)
        let mut stack: Vec<(usize, Path)> = vec![(start_offset, Path::empty())];

        while let Some((mut offset, mut parent_path)) = stack.pop() {
            loop {
                if offset + HEADER_SIZE > data.len() {
                    break;
                }

                // _PathItemHeader: PathIndex(4) + TokenIndex(4) + bits(1) + padding(3)
                let index = u32::from_le_bytes([
                    data[offset],
                    data[offset + 1],
                    data[offset + 2],
                    data[offset + 3],
                ]) as usize;

                let element_token_index = i32::from_le_bytes([
                    data[offset + 4],
                    data[offset + 5],
                    data[offset + 6],
                    data[offset + 7],
                ]);

                let bits = data[offset + 8];
                offset += HEADER_SIZE;

                if index >= self.paths.len() {
                    break;
                }

                // Build path
                if parent_path.is_empty() {
                    parent_path = Path::absolute_root();
                    self.paths[index] = parent_path.clone();
                } else {
                    let abs_token_idx = element_token_index.unsigned_abs() as usize;
                    if abs_token_idx < self.tokens.len() {
                        let elem_token = self.tokens[abs_token_idx].as_str();
                        let is_property = (bits & IS_PRIM_PROPERTY_PATH) != 0;
                        let new_path = if is_property {
                            parent_path.append_property(elem_token)
                        } else {
                            parent_path.append_element_token(&Token::new(elem_token))
                        };
                        if let Some(path) = new_path {
                            self.paths[index] = path;
                        }
                    }
                }

                let has_child = (bits & HAS_CHILD) != 0;
                let has_sibling = (bits & HAS_SIBLING) != 0;

                if has_child {
                    if has_sibling {
                        // Read sibling offset and push to stack for later
                        if offset + 8 <= data.len() {
                            let sibling_offset = i64::from_le_bytes([
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
                            stack.push((sibling_offset, parent_path.clone()));
                        }
                    }
                    // Continue with child - update parent path
                    parent_path = self.paths[index].clone();
                } else if !has_sibling {
                    break;
                }
                // If only has_sibling, continue with next item (parent unchanged)
            }
        }

        Ok(())
    }

    /// Reads specs section.
    fn read_specs(&mut self, data: &[u8]) -> Result<(), FileFormatError> {
        usd_trace::trace_scope!("usdc_read_specs");
        let Some(section) = self.toc.get_section(section_names::SPECS) else {
            return Ok(());
        };

        let start = section.start as usize;
        let size = section.size as usize;

        if start + size > data.len() {
            return Err(FileFormatError::corrupt_file(
                &self.asset_path,
                "Specs section truncated",
            ));
        }

        if size < 8 {
            return Ok(());
        }

        let count = Self::read_u64_at(data, start);
        let mut offset = start + 8;

        if self.version == (0, 0, 1) {
            // Oldest format: 16-byte specs
            self.specs.clear();
            self.specs.reserve(count);
            for _ in 0..count {
                if offset + CrateSpec::SIZE_0_0_1 > data.len() {
                    break;
                }
                let spec = CrateSpec::from_bytes_0_0_1(&data[offset..])?;
                self.specs.push(spec);
                offset += CrateSpec::SIZE_0_0_1;
            }
        } else if self.version < (0, 4, 0) {
            // Version >= 0.1.0 but < 0.4.0: 12-byte uncompressed specs
            self.specs.clear();
            self.specs.reserve(count);
            for _ in 0..count {
                if offset + CrateSpec::SIZE > data.len() {
                    break;
                }
                let spec = CrateSpec::from_bytes(&data[offset..])?;
                self.specs.push(spec);
                offset += CrateSpec::SIZE;
            }
        } else {
            // Version >= 0.4.0: three compressed u32 arrays (pathIndexes, fieldSetIndexes, specTypes)
            let (path_indexes, consumed) = Self::read_compressed_u32(
                data,
                offset,
                count,
                &self.asset_path,
                "spec path indexes",
            )?;
            offset += consumed;

            let (field_set_indexes, consumed) = Self::read_compressed_u32(
                data,
                offset,
                count,
                &self.asset_path,
                "spec field set indexes",
            )?;
            offset += consumed;

            let (spec_types, _consumed) =
                Self::read_compressed_u32(data, offset, count, &self.asset_path, "spec types")?;

            self.specs.clear();
            self.specs.reserve(count);
            for i in 0..count {
                self.specs.push(CrateSpec {
                    path_index: PathIndex(Index::new(path_indexes[i])),
                    field_set_index: FieldSetIndex(Index::new(field_set_indexes[i])),
                    spec_type: SpecType::from_u32(spec_types[i]),
                });
            }
        }

        Ok(())
    }

    /// Gets a token by index.
    #[must_use]
    pub fn get_token(&self, index: TokenIndex) -> Option<&Token> {
        self.tokens.get(index.0.value as usize)
    }

    /// Gets a string by index.
    #[must_use]
    pub fn get_string(&self, index: StringIndex) -> Option<&str> {
        let token_idx = self.strings.get(index.0.value as usize)?;
        self.get_token(*token_idx).map(|t| t.as_str())
    }

    /// Gets a path by index.
    #[must_use]
    pub fn get_path(&self, index: PathIndex) -> Option<&Path> {
        self.paths.get(index.0.value as usize)
    }

    /// Gets a field by index.
    #[must_use]
    pub fn get_field(&self, index: FieldIndex) -> Option<&Field> {
        self.fields.get(index.0.value as usize)
    }

    /// Gets a spec by index.
    #[must_use]
    pub fn get_spec(&self, index: usize) -> Option<&CrateSpec> {
        self.specs.get(index)
    }

    /// Returns the number of unique field sets.
    #[must_use]
    pub fn num_unique_field_sets(&self) -> usize {
        // Count terminators (invalid indices)
        self.field_sets
            .iter()
            .filter(|idx| !idx.0.is_valid())
            .count()
    }
}

impl Default for CrateFile {
    fn default() -> Self {
        Self::new()
    }
}
