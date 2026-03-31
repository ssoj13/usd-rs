//! Type definitions, constants, and binary format data structures for USDC.
//!
//! Contains: constants (USDC_MAGIC, SOFTWARE_VERSION, MIN_READ_VERSION),
//! Bootstrap, Section, TableOfContents, CrateHeader, SectionType, TypeEnum,
//! ValueRep, index types, Field, CrateSpec, CrateTimeSamples.

use crate::file_format::FileFormatError;
use crate::types::SpecType;

// ============================================================================
// Constants
// ============================================================================

/// Magic cookie for usdc files (8 bytes)
pub const USDC_MAGIC: &[u8; 8] = b"PXR-USDC";

/// Software version - matches crate format version
pub const SOFTWARE_VERSION: (u8, u8, u8) = (0, 9, 0);

/// Minimum supported version for reading
pub const MIN_READ_VERSION: (u8, u8, u8) = (0, 0, 1);

// ============================================================================
// Bootstrap (File Header)
// ============================================================================

/// Bootstrap size in bytes: 8 (ident) + 8 (version) + 8 (tocOffset) + 64 (reserved) = 88
pub const BOOTSTRAP_SIZE: usize = 88;

/// Section name maximum length (15 chars + null terminator)
pub const SECTION_NAME_MAX_LENGTH: usize = 15;

/// Section names as used in crate files.
pub mod section_names {
    /// Token table section name.
    pub const TOKENS: &str = "TOKENS";
    /// String table section name.
    pub const STRINGS: &str = "STRINGS";
    /// Fields section name.
    pub const FIELDS: &str = "FIELDS";
    /// Field sets section name.
    pub const FIELDSETS: &str = "FIELDSETS";
    /// Paths section name.
    pub const PATHS: &str = "PATHS";
    /// Specs section name.
    pub const SPECS: &str = "SPECS";
}

/// Bootstrap structure - appears at start of file.
///
/// Layout (88 bytes):
/// - ident[8]: "PXR-USDC"
/// - version[8]: major, minor, patch, rest unused
/// - tocOffset: i64 offset to TableOfContents
/// - reserved[8]: i64 reserved fields
#[derive(Debug, Clone)]
pub struct Bootstrap {
    /// File identifier ("PXR-USDC")
    pub ident: [u8; 8],
    /// Version bytes: [major, minor, patch, 0, 0, 0, 0, 0]
    pub version: [u8; 8],
    /// Offset to table of contents
    pub toc_offset: i64,
    /// Reserved fields
    pub reserved: [i64; 8],
}

impl Default for Bootstrap {
    fn default() -> Self {
        Self {
            ident: *USDC_MAGIC,
            version: [
                SOFTWARE_VERSION.0,
                SOFTWARE_VERSION.1,
                SOFTWARE_VERSION.2,
                0,
                0,
                0,
                0,
                0,
            ],
            toc_offset: 0,
            reserved: [0; 8],
        }
    }
}

impl Bootstrap {
    /// Creates bootstrap with specific version.
    pub fn with_version(major: u8, minor: u8, patch: u8) -> Self {
        Self {
            ident: *USDC_MAGIC,
            version: [major, minor, patch, 0, 0, 0, 0, 0],
            toc_offset: 0,
            reserved: [0; 8],
        }
    }

    /// Reads bootstrap from bytes.
    pub fn from_bytes(data: &[u8]) -> Result<Self, FileFormatError> {
        if data.len() < BOOTSTRAP_SIZE {
            return Err(FileFormatError::corrupt_file(
                "",
                format!("Bootstrap too short: {} < {}", data.len(), BOOTSTRAP_SIZE),
            ));
        }

        // Check magic
        if &data[0..8] != USDC_MAGIC {
            return Err(FileFormatError::corrupt_file(
                "",
                "Invalid magic, expected PXR-USDC".to_string(),
            ));
        }

        let mut ident = [0u8; 8];
        ident.copy_from_slice(&data[0..8]);

        let mut version = [0u8; 8];
        version.copy_from_slice(&data[8..16]);

        let toc_offset = i64::from_le_bytes([
            data[16], data[17], data[18], data[19], data[20], data[21], data[22], data[23],
        ]);

        let mut reserved = [0i64; 8];
        for i in 0..8 {
            let offset = 24 + i * 8;
            reserved[i] = i64::from_le_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
                data[offset + 4],
                data[offset + 5],
                data[offset + 6],
                data[offset + 7],
            ]);
        }

        Ok(Self {
            ident,
            version,
            toc_offset,
            reserved,
        })
    }

    /// Returns the version tuple (major, minor, patch).
    #[must_use]
    pub fn version_tuple(&self) -> (u8, u8, u8) {
        (self.version[0], self.version[1], self.version[2])
    }

    /// Writes bootstrap to bytes.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(BOOTSTRAP_SIZE);
        bytes.extend_from_slice(&self.ident);
        bytes.extend_from_slice(&self.version);
        bytes.extend_from_slice(&self.toc_offset.to_le_bytes());
        for &r in &self.reserved {
            bytes.extend_from_slice(&r.to_le_bytes());
        }
        bytes
    }
}

// ============================================================================
// Section
// ============================================================================

/// A section in the crate file.
///
/// Layout (32 bytes):
/// - name[16]: null-terminated section name
/// - start: i64 offset from file start
/// - size: i64 size in bytes
#[derive(Debug, Clone)]
pub struct Section {
    /// Section name (max 15 chars + null)
    pub name: String,
    /// Start offset in file
    pub start: i64,
    /// Size in bytes
    pub size: i64,
}

impl Section {
    /// Section size in bytes when serialized.
    pub const SIZE: usize = 32; // 16 (name) + 8 (start) + 8 (size)

    /// Creates a new section.
    pub fn new(name: &str, start: i64, size: i64) -> Self {
        Self {
            name: name.to_string(),
            start,
            size,
        }
    }

    /// Reads section from bytes.
    pub fn from_bytes(data: &[u8]) -> Result<Self, FileFormatError> {
        if data.len() < Self::SIZE {
            return Err(FileFormatError::corrupt_file("", "Section data too short"));
        }

        // Read name (null-terminated)
        let name_bytes = &data[0..16];
        let name_end = name_bytes.iter().position(|&b| b == 0).unwrap_or(16);
        let name = String::from_utf8_lossy(&name_bytes[..name_end]).to_string();

        let start = i64::from_le_bytes([
            data[16], data[17], data[18], data[19], data[20], data[21], data[22], data[23],
        ]);

        let size = i64::from_le_bytes([
            data[24], data[25], data[26], data[27], data[28], data[29], data[30], data[31],
        ]);

        Ok(Self { name, start, size })
    }

    /// Writes section to bytes.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(Self::SIZE);

        // Write name (padded to 16 bytes)
        let name_bytes = self.name.as_bytes();
        let copy_len = name_bytes.len().min(SECTION_NAME_MAX_LENGTH);
        bytes.extend_from_slice(&name_bytes[..copy_len]);
        bytes.resize(16, 0); // Pad with zeros

        bytes.extend_from_slice(&self.start.to_le_bytes());
        bytes.extend_from_slice(&self.size.to_le_bytes());

        bytes
    }
}

// ============================================================================
// TableOfContents
// ============================================================================

/// Table of contents listing all sections in the file.
#[derive(Debug, Clone, Default)]
pub struct TableOfContents {
    /// List of sections
    pub sections: Vec<Section>,
}

impl TableOfContents {
    /// Creates an empty table of contents.
    pub fn new() -> Self {
        Self::default()
    }

    /// Gets a section by name.
    pub fn get_section(&self, name: &str) -> Option<&Section> {
        self.sections.iter().find(|s| s.name == name)
    }

    /// Adds a section to the table of contents.
    pub fn add_section(&mut self, section: Section) {
        self.sections.push(section);
    }

    /// Gets the minimum section start offset.
    pub fn min_section_start(&self) -> i64 {
        self.sections.iter().map(|s| s.start).min().unwrap_or(0)
    }

    /// Reads table of contents from bytes.
    pub fn from_bytes(data: &[u8], num_sections: usize) -> Result<Self, FileFormatError> {
        let required_size = num_sections * Section::SIZE;
        if data.len() < required_size {
            return Err(FileFormatError::corrupt_file(
                "",
                format!("ToC data too short: {} < {}", data.len(), required_size),
            ));
        }

        let mut sections = Vec::with_capacity(num_sections);
        for i in 0..num_sections {
            let offset = i * Section::SIZE;
            let section = Section::from_bytes(&data[offset..])?;
            sections.push(section);
        }

        Ok(Self { sections })
    }

    /// Writes table of contents to bytes.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(self.sections.len() * Section::SIZE);
        for section in &self.sections {
            bytes.extend_from_slice(&section.to_bytes());
        }
        bytes
    }
}

// ============================================================================
// CrateHeader (legacy alias for compatibility)
// ============================================================================

/// Crate file header (alias for Bootstrap).
#[derive(Debug, Clone)]
pub struct CrateHeader {
    /// Version major
    pub version_major: u8,
    /// Version minor
    pub version_minor: u8,
    /// Version patch
    pub version_patch: u8,
    /// Number of sections
    pub num_sections: u32,
    /// Table of contents offset
    pub toc_offset: u64,
}

impl CrateHeader {
    /// Reads header from bytes (parses Bootstrap and extracts key fields).
    pub fn from_bytes(data: &[u8]) -> Result<Self, FileFormatError> {
        let bootstrap = Bootstrap::from_bytes(data)?;
        let (major, minor, patch) = bootstrap.version_tuple();

        Ok(Self {
            version_major: major,
            version_minor: minor,
            version_patch: patch,
            num_sections: 0, // Will be determined from ToC
            toc_offset: bootstrap.toc_offset as u64,
        })
    }

    /// Returns the version tuple.
    #[must_use]
    pub fn version(&self) -> (u8, u8, u8) {
        (self.version_major, self.version_minor, self.version_patch)
    }

    /// Writes header to bytes.
    pub fn to_bytes(&self) -> Vec<u8> {
        let bootstrap = Bootstrap {
            ident: *USDC_MAGIC,
            version: [
                self.version_major,
                self.version_minor,
                self.version_patch,
                0,
                0,
                0,
                0,
                0,
            ],
            toc_offset: self.toc_offset as i64,
            reserved: [0; 8],
        };
        bootstrap.to_bytes()
    }
}

// ============================================================================
// Section Types
// ============================================================================

/// Section types in a crate file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum SectionType {
    /// Unknown section
    Unknown = 0,
    /// Token table
    Tokens = 1,
    /// String table
    Strings = 2,
    /// Field data
    Fields = 3,
    /// Field set data
    FieldSets = 4,
    /// Path data
    Paths = 5,
    /// Spec data
    Specs = 6,
}

impl From<u8> for SectionType {
    fn from(value: u8) -> Self {
        match value {
            1 => Self::Tokens,
            2 => Self::Strings,
            3 => Self::Fields,
            4 => Self::FieldSets,
            5 => Self::Paths,
            6 => Self::Specs,
            _ => Self::Unknown,
        }
    }
}

// ============================================================================
// TypeEnum - Value type enumeration
// ============================================================================

/// Value type enumeration for crate format.
///
/// These values are stored in the upper byte of ValueRep and identify
/// the type of value stored. Note that 0 is reserved for Invalid.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(i32)]
pub enum TypeEnum {
    /// Invalid/uninitialized type
    Invalid = 0,

    // Array-capable types (supportsArray = true)
    /// Boolean value
    Bool = 1,
    /// Unsigned 8-bit integer
    UChar = 2,
    /// Signed 32-bit integer
    Int = 3,
    /// Unsigned 32-bit integer
    UInt = 4,
    /// Signed 64-bit integer
    Int64 = 5,
    /// Unsigned 64-bit integer
    UInt64 = 6,
    /// 16-bit floating point (half)
    Half = 7,
    /// 32-bit floating point
    Float = 8,
    /// 64-bit floating point
    Double = 9,
    /// String value
    String = 10,
    /// Token value
    Token = 11,
    /// Asset path value
    AssetPath = 12,
    /// 2x2 double matrix
    Matrix2d = 13,
    /// 3x3 double matrix
    Matrix3d = 14,
    /// 4x4 double matrix
    Matrix4d = 15,
    /// Double quaternion
    Quatd = 16,
    /// Float quaternion
    Quatf = 17,
    /// Half quaternion
    Quath = 18,
    /// 2D double vector
    Vec2d = 19,
    /// 2D float vector
    Vec2f = 20,
    /// 2D half vector
    Vec2h = 21,
    /// 2D integer vector
    Vec2i = 22,
    /// 3D double vector
    Vec3d = 23,
    /// 3D float vector
    Vec3f = 24,
    /// 3D half vector
    Vec3h = 25,
    /// 3D integer vector
    Vec3i = 26,
    /// 4D double vector
    Vec4d = 27,
    /// 4D float vector
    Vec4f = 28,
    /// 4D half vector
    Vec4h = 29,
    /// 4D integer vector
    Vec4i = 30,

    // Non-array types (supportsArray = false)
    /// Dictionary (VtDictionary)
    Dictionary = 31,
    /// Token list operation
    TokenListOp = 32,
    /// String list operation
    StringListOp = 33,
    /// Path list operation
    PathListOp = 34,
    /// Reference list operation
    ReferenceListOp = 35,
    /// Integer list operation
    IntListOp = 36,
    /// Int64 list operation
    Int64ListOp = 37,
    /// Unsigned integer list operation
    UIntListOp = 38,
    /// UInt64 list operation
    UInt64ListOp = 39,
    /// Path vector
    PathVector = 40,
    /// Token vector
    TokenVector = 41,
    /// Specifier (def, over, class)
    Specifier = 42,
    /// Permission
    Permission = 43,
    /// Variability (uniform, varying)
    Variability = 44,
    /// Variant selection map
    VariantSelectionMap = 45,
    /// Time samples
    TimeSamples = 46,
    /// Payload
    Payload = 47,
    /// Double vector
    DoubleVector = 48,
    /// Layer offset vector
    LayerOffsetVector = 49,
    /// String vector
    StringVector = 50,
    /// Value block
    ValueBlock = 51,
    /// Generic value (VtValue)
    Value = 52,
    /// Unregistered value
    UnregisteredValue = 53,
    /// Unregistered value list operation
    UnregisteredValueListOp = 54,
    /// Payload list operation
    PayloadListOp = 55,

    // Array-capable types (added later with higher enum values)
    /// Time code
    TimeCode = 56,
    /// Path expression
    PathExpression = 57,

    // More non-array types
    /// Relocates
    Relocates = 58,
    /// Spline
    Spline = 59,
    /// Animation block
    AnimationBlock = 60,
}

impl TypeEnum {
    /// Total number of type enum values.
    pub const NUM_TYPES: i32 = 61;

    /// Returns whether this type supports arrays.
    #[must_use]
    pub fn supports_array(self) -> bool {
        matches!(
            self,
            Self::Bool
                | Self::UChar
                | Self::Int
                | Self::UInt
                | Self::Int64
                | Self::UInt64
                | Self::Half
                | Self::Float
                | Self::Double
                | Self::String
                | Self::Token
                | Self::AssetPath
                | Self::Matrix2d
                | Self::Matrix3d
                | Self::Matrix4d
                | Self::Quatd
                | Self::Quatf
                | Self::Quath
                | Self::Vec2d
                | Self::Vec2f
                | Self::Vec2h
                | Self::Vec2i
                | Self::Vec3d
                | Self::Vec3f
                | Self::Vec3h
                | Self::Vec3i
                | Self::Vec4d
                | Self::Vec4f
                | Self::Vec4h
                | Self::Vec4i
                | Self::TimeCode
                | Self::PathExpression
        )
    }

    /// Creates TypeEnum from raw value.
    #[must_use]
    pub fn from_raw(value: i32) -> Self {
        match value {
            0 => Self::Invalid,
            1 => Self::Bool,
            2 => Self::UChar,
            3 => Self::Int,
            4 => Self::UInt,
            5 => Self::Int64,
            6 => Self::UInt64,
            7 => Self::Half,
            8 => Self::Float,
            9 => Self::Double,
            10 => Self::String,
            11 => Self::Token,
            12 => Self::AssetPath,
            13 => Self::Matrix2d,
            14 => Self::Matrix3d,
            15 => Self::Matrix4d,
            16 => Self::Quatd,
            17 => Self::Quatf,
            18 => Self::Quath,
            19 => Self::Vec2d,
            20 => Self::Vec2f,
            21 => Self::Vec2h,
            22 => Self::Vec2i,
            23 => Self::Vec3d,
            24 => Self::Vec3f,
            25 => Self::Vec3h,
            26 => Self::Vec3i,
            27 => Self::Vec4d,
            28 => Self::Vec4f,
            29 => Self::Vec4h,
            30 => Self::Vec4i,
            31 => Self::Dictionary,
            32 => Self::TokenListOp,
            33 => Self::StringListOp,
            34 => Self::PathListOp,
            35 => Self::ReferenceListOp,
            36 => Self::IntListOp,
            37 => Self::Int64ListOp,
            38 => Self::UIntListOp,
            39 => Self::UInt64ListOp,
            40 => Self::PathVector,
            41 => Self::TokenVector,
            42 => Self::Specifier,
            43 => Self::Permission,
            44 => Self::Variability,
            45 => Self::VariantSelectionMap,
            46 => Self::TimeSamples,
            47 => Self::Payload,
            48 => Self::DoubleVector,
            49 => Self::LayerOffsetVector,
            50 => Self::StringVector,
            51 => Self::ValueBlock,
            52 => Self::Value,
            53 => Self::UnregisteredValue,
            54 => Self::UnregisteredValueListOp,
            55 => Self::PayloadListOp,
            56 => Self::TimeCode,
            57 => Self::PathExpression,
            58 => Self::Relocates,
            59 => Self::Spline,
            60 => Self::AnimationBlock,
            _ => Self::Invalid,
        }
    }
}

impl From<i32> for TypeEnum {
    fn from(value: i32) -> Self {
        Self::from_raw(value)
    }
}

impl From<TypeEnum> for i32 {
    fn from(value: TypeEnum) -> Self {
        value as i32
    }
}

// ============================================================================
// ValueRep - Value representation in file
// ============================================================================

/// Value representation in crate file format.
///
/// This is an 8-byte value that encodes type information and either
/// an inline value or a file offset to the value data.
///
/// Layout (64 bits):
/// - Bit 63: IsArray flag
/// - Bit 62: IsInlined flag
/// - Bit 61: IsCompressed flag
/// - Bit 60: IsArrayEdit flag
/// - Bits 48-55: Type enum value (8 bits)
/// - Bits 0-47: Payload (48 bits) - inline value or file offset
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct ValueRep {
    /// Raw 64-bit data
    pub data: u64,
}

impl ValueRep {
    /// Bit position for IsArray flag
    pub const IS_ARRAY_BIT: u64 = 1 << 63;
    /// Bit position for IsInlined flag
    pub const IS_INLINED_BIT: u64 = 1 << 62;
    /// Bit position for IsCompressed flag
    pub const IS_COMPRESSED_BIT: u64 = 1 << 61;
    /// Bit position for IsArrayEdit flag
    pub const IS_ARRAY_EDIT_BIT: u64 = 1 << 60;
    /// Mask for extracting payload (lower 48 bits)
    pub const PAYLOAD_MASK: u64 = (1 << 48) - 1;

    /// Creates a new ValueRep from raw data.
    #[must_use]
    pub const fn from_raw(data: u64) -> Self {
        Self { data }
    }

    /// Creates a new ValueRep with specified components.
    #[must_use]
    pub const fn new(type_enum: TypeEnum, is_inlined: bool, is_array: bool, payload: u64) -> Self {
        let mut data = 0u64;
        if is_array {
            data |= Self::IS_ARRAY_BIT;
        }
        if is_inlined {
            data |= Self::IS_INLINED_BIT;
        }
        data |= ((type_enum as u64) & 0xFF) << 48;
        data |= payload & Self::PAYLOAD_MASK;
        Self { data }
    }

    /// Creates a new inlined ValueRep for a scalar value.
    #[must_use]
    pub const fn new_inlined(type_enum: TypeEnum, payload: u32) -> Self {
        Self::new(type_enum, true, false, payload as u64)
    }

    /// Creates a new ValueRep pointing to data at file offset.
    #[must_use]
    pub const fn new_at_offset(type_enum: TypeEnum, offset: u64) -> Self {
        Self::new(type_enum, false, false, offset)
    }

    /// Creates a new array ValueRep pointing to data at file offset.
    #[must_use]
    pub const fn new_array_at_offset(type_enum: TypeEnum, offset: u64) -> Self {
        Self::new(type_enum, false, true, offset)
    }

    /// Returns whether this represents an array value.
    #[must_use]
    pub const fn is_array(&self) -> bool {
        (self.data & Self::IS_ARRAY_BIT) != 0
    }

    /// Sets the array flag.
    pub fn set_is_array(&mut self) {
        self.data |= Self::IS_ARRAY_BIT;
    }

    /// Returns whether the value is inlined in the payload.
    #[must_use]
    pub const fn is_inlined(&self) -> bool {
        (self.data & Self::IS_INLINED_BIT) != 0
    }

    /// Sets the inlined flag.
    pub fn set_is_inlined(&mut self) {
        self.data |= Self::IS_INLINED_BIT;
    }

    /// Returns whether the data is compressed.
    #[must_use]
    pub const fn is_compressed(&self) -> bool {
        (self.data & Self::IS_COMPRESSED_BIT) != 0
    }

    /// Sets the compressed flag.
    pub fn set_is_compressed(&mut self) {
        self.data |= Self::IS_COMPRESSED_BIT;
    }

    /// Returns whether this is an array edit operation.
    #[must_use]
    pub const fn is_array_edit(&self) -> bool {
        (self.data & Self::IS_ARRAY_EDIT_BIT) != 0
    }

    /// Sets the array edit flag.
    pub fn set_is_array_edit(&mut self) {
        self.data |= Self::IS_ARRAY_EDIT_BIT;
    }

    /// Returns the type enum value.
    #[must_use]
    pub fn get_type(&self) -> TypeEnum {
        TypeEnum::from_raw(((self.data >> 48) & 0xFF) as i32)
    }

    /// Sets the type enum value.
    pub fn set_type(&mut self, t: TypeEnum) {
        self.data &= !(0xFF << 48); // Clear type byte
        self.data |= ((t as u64) & 0xFF) << 48; // Set new type
    }

    /// Returns the payload value (lower 48 bits).
    #[must_use]
    pub const fn get_payload(&self) -> u64 {
        self.data & Self::PAYLOAD_MASK
    }

    /// Sets the payload value.
    pub fn set_payload(&mut self, payload: u64) {
        self.data &= !Self::PAYLOAD_MASK; // Clear payload
        self.data |= payload & Self::PAYLOAD_MASK; // Set new payload
    }

    /// Returns the raw 64-bit data.
    #[must_use]
    pub const fn get_data(&self) -> u64 {
        self.data
    }

    /// Reads ValueRep from bytes (little-endian).
    #[must_use]
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < 8 {
            return None;
        }
        let data = u64::from_le_bytes([
            bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
        ]);
        Some(Self { data })
    }

    /// Writes ValueRep to bytes (little-endian).
    #[must_use]
    pub fn to_bytes(&self) -> [u8; 8] {
        self.data.to_le_bytes()
    }
}

impl From<u64> for ValueRep {
    fn from(data: u64) -> Self {
        Self { data }
    }
}

impl From<ValueRep> for u64 {
    fn from(rep: ValueRep) -> Self {
        rep.data
    }
}

// ============================================================================
// Index types for various tables
// ============================================================================

/// Base index type for table indexing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Index {
    /// Index value (u32::MAX means invalid)
    pub value: u32,
}

impl Index {
    /// Invalid index marker
    pub const INVALID: u32 = u32::MAX;

    /// Creates an invalid index.
    #[must_use]
    pub const fn invalid() -> Self {
        Self {
            value: Self::INVALID,
        }
    }

    /// Creates an index with the given value.
    #[must_use]
    pub const fn new(value: u32) -> Self {
        Self { value }
    }

    /// Returns whether this index is valid.
    #[must_use]
    pub const fn is_valid(&self) -> bool {
        self.value != Self::INVALID
    }
}

/// Field table index.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct FieldIndex(pub Index);

impl FieldIndex {
    /// Creates a new field index.
    #[must_use]
    pub const fn new(value: u32) -> Self {
        Self(Index::new(value))
    }

    /// Creates an invalid field index.
    #[must_use]
    pub const fn invalid() -> Self {
        Self(Index::invalid())
    }

    /// Returns the raw u32 value.
    #[must_use]
    pub const fn value(&self) -> u32 {
        self.0.value
    }
}

/// Field set table index.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct FieldSetIndex(pub Index);

impl FieldSetIndex {
    /// Creates a new field set index.
    #[must_use]
    pub const fn new(value: u32) -> Self {
        Self(Index::new(value))
    }

    /// Returns the raw u32 value.
    #[must_use]
    pub const fn value(&self) -> u32 {
        self.0.value
    }
}

/// Path table index.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct PathIndex(pub Index);

impl PathIndex {
    /// Creates a new path index.
    #[must_use]
    pub const fn new(value: u32) -> Self {
        Self(Index::new(value))
    }

    /// Returns the raw u32 value.
    #[must_use]
    pub const fn value(&self) -> u32 {
        self.0.value
    }
}

/// String table index.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct StringIndex(pub Index);

impl StringIndex {
    /// Creates a new string index.
    #[must_use]
    pub const fn new(value: u32) -> Self {
        Self(Index::new(value))
    }

    /// Returns the raw u32 value.
    #[must_use]
    pub const fn value(&self) -> u32 {
        self.0.value
    }
}

/// Token table index.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct TokenIndex(pub Index);

impl TokenIndex {
    /// Creates a new token index.
    #[must_use]
    pub const fn new(value: u32) -> Self {
        Self(Index::new(value))
    }

    /// Returns the raw u32 value.
    #[must_use]
    pub const fn value(&self) -> u32 {
        self.0.value
    }
}

// ============================================================================
// Field - Token/Value pair in crate format
// ============================================================================

/// A field in crate format - maps a token index to a value representation.
///
/// Layout (16 bytes):
/// - 4 bytes padding (historical bug compatibility)
/// - 4 bytes token index
/// - 8 bytes value representation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Field {
    /// Padding for historical compatibility
    pub padding: u32,
    /// Index into token table
    pub token_index: TokenIndex,
    /// Value representation
    pub value_rep: ValueRep,
}

impl Field {
    /// Field size in bytes when serialized.
    pub const SIZE: usize = 16;

    /// Creates a new field.
    #[must_use]
    pub const fn new(token_index: TokenIndex, value_rep: ValueRep) -> Self {
        Self {
            padding: 0,
            token_index,
            value_rep,
        }
    }

    /// Reads field from bytes.
    pub fn from_bytes(data: &[u8]) -> Result<Self, FileFormatError> {
        if data.len() < Self::SIZE {
            return Err(FileFormatError::corrupt_file("", "Field data too short"));
        }

        let padding = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
        let token_value = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);
        let value_rep = ValueRep::from_bytes(&data[8..]).expect("valid value rep");

        Ok(Self {
            padding,
            token_index: TokenIndex::new(token_value),
            value_rep,
        })
    }

    /// Writes field to bytes.
    #[must_use]
    pub fn to_bytes(&self) -> [u8; Self::SIZE] {
        let mut bytes = [0u8; Self::SIZE];
        bytes[0..4].copy_from_slice(&self.padding.to_le_bytes());
        bytes[4..8].copy_from_slice(&self.token_index.0.value.to_le_bytes());
        bytes[8..16].copy_from_slice(&self.value_rep.to_bytes());
        bytes
    }
}

// ============================================================================
// Helper: FromLeBytes trait for generic integer reading
// ============================================================================

/// Trait for reading values from little-endian bytes.
pub(super) trait FromLeBytes {
    fn from_le_bytes(data: &[u8]) -> Self;
}

impl FromLeBytes for i32 {
    fn from_le_bytes(data: &[u8]) -> Self {
        i32::from_le_bytes([data[0], data[1], data[2], data[3]])
    }
}

impl FromLeBytes for u32 {
    fn from_le_bytes(data: &[u8]) -> Self {
        u32::from_le_bytes([data[0], data[1], data[2], data[3]])
    }
}

impl FromLeBytes for i64 {
    fn from_le_bytes(data: &[u8]) -> Self {
        i64::from_le_bytes([
            data[0], data[1], data[2], data[3], data[4], data[5], data[6], data[7],
        ])
    }
}

impl FromLeBytes for u64 {
    fn from_le_bytes(data: &[u8]) -> Self {
        u64::from_le_bytes([
            data[0], data[1], data[2], data[3], data[4], data[5], data[6], data[7],
        ])
    }
}

impl FromLeBytes for f64 {
    fn from_le_bytes(data: &[u8]) -> Self {
        f64::from_bits(u64::from_le_bytes([
            data[0], data[1], data[2], data[3], data[4], data[5], data[6], data[7],
        ]))
    }
}

// ============================================================================
// Helper: Half-float conversion
// ============================================================================

/// Converts a half-precision (16-bit) float to f32.
/// IEEE 754 half-precision: 1 sign + 5 exponent + 10 mantissa bits.
#[inline]
pub(super) fn half_to_f32(h: u16) -> f32 {
    let sign = ((h >> 15) & 1) as u32;
    let exp = ((h >> 10) & 0x1F) as u32;
    let mant = (h & 0x3FF) as u32;

    if exp == 0 {
        if mant == 0 {
            // Zero (positive or negative)
            f32::from_bits(sign << 31)
        } else {
            // Denormalized - convert to normalized f32
            let mut m = mant;
            let mut e: i32 = 1;
            while (m & 0x400) == 0 {
                m <<= 1;
                e -= 1;
            }
            m &= 0x3FF;
            let new_exp = (127 - 15 + e) as u32;
            f32::from_bits((sign << 31) | (new_exp << 23) | (m << 13))
        }
    } else if exp == 31 {
        // Infinity or NaN
        if mant == 0 {
            f32::from_bits((sign << 31) | 0x7F80_0000)
        } else {
            f32::from_bits((sign << 31) | 0x7FC0_0000 | (mant << 13))
        }
    } else {
        // Normalized
        let new_exp = exp + (127 - 15);
        f32::from_bits((sign << 31) | (new_exp << 23) | (mant << 13))
    }
}

// ============================================================================
// Helper: SpecType conversion
// ============================================================================

/// Converts a u32 value to SpecType.
pub(super) fn spec_type_to_enum(value: u32) -> SpecType {
    match value {
        0 => SpecType::Unknown,
        1 => SpecType::Attribute,
        2 => SpecType::Connection,
        3 => SpecType::Expression,
        4 => SpecType::Mapper,
        5 => SpecType::MapperArg,
        6 => SpecType::Prim,
        7 => SpecType::PseudoRoot,
        8 => SpecType::Relationship,
        9 => SpecType::RelationshipTarget,
        10 => SpecType::Variant,
        11 => SpecType::VariantSet,
        _ => SpecType::Unknown,
    }
}

// ============================================================================
// Spec - Specification entry in crate format
// ============================================================================

/// A spec entry in crate format.
///
/// Layout (12 bytes for version >= 0.1.0):
/// - 4 bytes path index
/// - 4 bytes field set index
/// - 4 bytes spec type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CrateSpec {
    /// Index into path table
    pub path_index: PathIndex,
    /// Index into field set table
    pub field_set_index: FieldSetIndex,
    /// Spec type
    pub spec_type: SpecType,
}

impl CrateSpec {
    /// Spec size in bytes (version >= 0.1.0).
    pub const SIZE: usize = 12;
    /// Spec size in bytes (version 0.0.1).
    pub const SIZE_0_0_1: usize = 16;

    /// Creates a new spec.
    #[must_use]
    pub const fn new(
        path_index: PathIndex,
        field_set_index: FieldSetIndex,
        spec_type: SpecType,
    ) -> Self {
        Self {
            path_index,
            field_set_index,
            spec_type,
        }
    }

    /// Reads spec from bytes (version >= 0.1.0).
    pub fn from_bytes(data: &[u8]) -> Result<Self, FileFormatError> {
        if data.len() < Self::SIZE {
            return Err(FileFormatError::corrupt_file("", "Spec data too short"));
        }

        let path_value = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
        let field_set_value = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);
        let spec_type_value = u32::from_le_bytes([data[8], data[9], data[10], data[11]]);

        Ok(Self {
            path_index: PathIndex::new(path_value),
            field_set_index: FieldSetIndex::new(field_set_value),
            spec_type: spec_type_to_enum(spec_type_value),
        })
    }

    /// Reads spec from bytes (version 0.0.1 with padding).
    pub fn from_bytes_0_0_1(data: &[u8]) -> Result<Self, FileFormatError> {
        if data.len() < Self::SIZE_0_0_1 {
            return Err(FileFormatError::corrupt_file(
                "",
                "Spec data too short (0.0.1)",
            ));
        }

        // Skip 4 bytes padding
        let path_value = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);
        let field_set_value = u32::from_le_bytes([data[8], data[9], data[10], data[11]]);
        let spec_type_value = u32::from_le_bytes([data[12], data[13], data[14], data[15]]);

        Ok(Self {
            path_index: PathIndex::new(path_value),
            field_set_index: FieldSetIndex::new(field_set_value),
            spec_type: spec_type_to_enum(spec_type_value),
        })
    }

    /// Writes spec to bytes.
    #[must_use]
    pub fn to_bytes(&self) -> [u8; Self::SIZE] {
        let mut bytes = [0u8; Self::SIZE];
        bytes[0..4].copy_from_slice(&self.path_index.0.value.to_le_bytes());
        bytes[4..8].copy_from_slice(&self.field_set_index.0.value.to_le_bytes());
        bytes[8..12].copy_from_slice(&(self.spec_type as u32).to_le_bytes());
        bytes
    }
}

// ============================================================================
// TimeSamples - Time sample storage in crate format
// ============================================================================

/// Time samples storage for crate format.
///
/// Stores time-value pairs with optional lazy loading from file.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CrateTimeSamples {
    /// Original value rep from file (0 if not from file or modified)
    pub value_rep: ValueRep,
    /// Sample times
    pub times: Vec<f64>,
    /// Sample values (only if in-memory)
    pub values: Vec<usd_vt::Value>,
    /// File offset for lazy loading (0 if in-memory)
    pub values_file_offset: i64,
}

impl CrateTimeSamples {
    /// Creates empty time samples.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns whether samples are in memory (not lazy-loaded).
    #[must_use]
    pub fn is_in_memory(&self) -> bool {
        self.value_rep.get_data() == 0
    }

    /// Returns the number of samples.
    #[must_use]
    pub fn len(&self) -> usize {
        self.times.len()
    }

    /// Returns whether there are no samples.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.times.is_empty()
    }
}
