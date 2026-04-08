//! Core types for Hydra.
//!
//! This module defines fundamental types used throughout Hydra including:
//! - Dirty bits for change tracking
//! - Tuple types for primvar data
//! - Sampler parameters for texturing
//! - Packed data types for efficient storage

use crate::enums::{HdBorderColor, HdCompareFunction, HdMagFilter, HdMinFilter, HdWrap};
use usd_gf::{
    Matrix3d, Matrix3f, Matrix4d, Matrix4f, Quatd, Quatf, Quath, Vec2d, Vec2f, Vec2i, Vec3d, Vec3f,
    Vec3i, Vec4d, Vec4f, Vec4i,
};
use usd_vt::Value;

/// Type representing a set of dirty bits.
///
/// Dirty bits are used to track what aspects of a prim have changed and
/// need to be re-synced with the renderer. Each bit represents a specific
/// aspect of the prim (e.g., transform, visibility, topology).
pub type HdDirtyBits = u32;

/// Compact representation of a 4-component vector using packed integer format.
///
/// `HdVec4_2_10_10_10_Rev` uses 10 bits for x, y, and z components, and 2 bits
/// for the w component, all packed into a single 32-bit integer. This provides
/// a memory-efficient representation for normalized vector data.
///
/// The layout is reverse-order (little-endian): x(10) | y(10) | z(10) | w(2)
///
/// This corresponds to `GL_INT_2_10_10_10_REV` format and `HdVec4f_2_10_10_10_REV`
/// in the C++ OpenUSD implementation.
///
/// # Note
///
/// This type is expected to move as work continues on refactoring GL dependencies.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct HdVec4_2_10_10_10_Rev {
    /// Packed 32-bit representation of the 4-component vector.
    data: i32,
}

impl HdVec4_2_10_10_10_Rev {
    /// Creates a new packed vector from a raw 32-bit integer value.
    ///
    /// # Arguments
    ///
    /// * `value` - Raw 32-bit integer containing the packed vector data.
    pub fn from_i32(value: i32) -> Self {
        Self { data: value }
    }

    /// Extracts the raw 32-bit integer representation.
    ///
    /// # Returns
    ///
    /// The packed vector data as a 32-bit integer.
    pub fn as_i32(self) -> i32 {
        self.data
    }

    /// Creates a packed vector from three float components.
    ///
    /// Input values are clamped to the range [-1.0, 1.0] and converted to
    /// 10-bit signed fixed-point integers using the GL spec equation.
    ///
    /// # Arguments
    ///
    /// * `x` - X component (clamped to [-1.0, 1.0])
    /// * `y` - Y component (clamped to [-1.0, 1.0])
    /// * `z` - Z component (clamped to [-1.0, 1.0])
    ///
    /// # Returns
    ///
    /// A new packed vector with the w component set to 0.
    pub fn from_vec3(x: f32, y: f32, z: f32) -> Self {
        let x_fixed = convert_float_to_fixed(x, 10);
        let y_fixed = convert_float_to_fixed(y, 10);
        let z_fixed = convert_float_to_fixed(z, 10);

        // Pack into 32 bits: w(2) | z(10) | y(10) | x(10)
        let packed = (x_fixed & 0x3FF) | ((y_fixed & 0x3FF) << 10) | ((z_fixed & 0x3FF) << 20);

        Self { data: packed }
    }

    /// Extracts the packed components as floating-point values.
    ///
    /// Unpacks the 10-bit signed integers and converts them back to floats
    /// in the range [-1.0, 1.0] using the GL spec equation.
    ///
    /// # Returns
    ///
    /// A tuple of (x, y, z) float values.
    pub fn to_vec3(self) -> (f32, f32, f32) {
        // Extract 10-bit signed values
        let x = sign_extend(self.data & 0x3FF, 10);
        let y = sign_extend((self.data >> 10) & 0x3FF, 10);
        let z = sign_extend((self.data >> 20) & 0x3FF, 10);

        let x_float = convert_fixed_to_float(x, 10);
        let y_float = convert_fixed_to_float(y, 10);
        let z_float = convert_fixed_to_float(z, 10);

        (x_float, y_float, z_float)
    }
}

/// Converts a float in range [-1.0, 1.0] to a fixed-point signed integer.
///
/// This implements the OpenGL specification 2.3.5.2 equation 2.4 for signed values:
/// `f(x) = round(clamp(x, -1, 1) * (2^(bits-1) - 1))`
///
/// # Arguments
///
/// * `v` - Float value to convert (will be clamped to [-1.0, 1.0])
/// * `bits` - Number of bits in the fixed-point representation
///
/// # Returns
///
/// Signed integer representation of the float value.
fn convert_float_to_fixed(v: f32, bits: u32) -> i32 {
    let clamped = v.clamp(-1.0, 1.0);
    let scale = ((1 << (bits - 1)) - 1) as f32;
    (clamped * scale).round() as i32
}

/// Converts a fixed-point signed integer back to a float.
///
/// This implements the OpenGL specification 2.3.5.1 equation 2.2 for signed values:
/// `f(x) = max(-1.0, x / (2^(bits-1) - 1))`
///
/// # Arguments
///
/// * `v` - Fixed-point integer value to convert
/// * `bits` - Number of bits in the fixed-point representation
///
/// # Returns
///
/// Float value in range [-1.0, 1.0].
fn convert_fixed_to_float(v: i32, bits: u32) -> f32 {
    let scale = ((1 << (bits - 1)) - 1) as f32;
    (v as f32 / scale).max(-1.0)
}

/// Sign-extends a value from N bits to 32 bits.
///
/// Preserves the sign bit when extending a smaller signed integer
/// to a full 32-bit signed integer.
///
/// # Arguments
///
/// * `value` - The value to sign-extend
/// * `bits` - Number of bits in the original value
///
/// # Returns
///
/// Sign-extended 32-bit integer.
fn sign_extend(value: i32, bits: u32) -> i32 {
    let shift = 32 - bits;
    (value << shift) >> shift
}

/// HdType describes the type of an attribute value used in Hydra.
///
/// HdType values have a specific machine representation and size.
/// See [`HdType::size_in_bytes()`] for size information.
///
/// HdType specifies a scalar, vector, or matrix type. Vector and
/// matrix types can be unpacked into the underlying "component"
/// type; see [`HdType::component_type()`].
///
/// HdType is intended to span the common set of attribute types
/// used in shading languages such as GLSL. However, it currently
/// does not include non-4x4 matrix types, nor struct types.
///
/// Fixed-size array types are represented by the related struct
/// [`HdTupleType`]. HdTupleType is used anywhere there is a
/// possibility of an array of values.
///
/// # Value arrays and attribute buffers
///
/// Attribute data is often stored in linear buffers. These buffers
/// have multiple dimensions and it is important to distinguish them:
///
/// - **Components** refer to the scalar components that comprise a vector
///   or matrix. For example, a vec3 has 3 components, a mat4 has
///   16 components, and a float has a single component.
///
/// - **Elements** refer to external concepts that entries in a buffer
///   associate with. Typically these are pieces of geometry,
///   such as faces or vertices.
///
/// - **Arrays** refer to the idea that each element may associate
///   with a fixed-size array of values. For example, one approach
///   to motion blur might store a size-2 array of `HdFloatMat4`
///   values for each element of geometry, holding the transforms
///   at the beginning and ending of the camera shutter interval.
///
/// Combining these concepts in an example, a primvar buffer might hold
/// data for 10 vertices (the elements) with each vertex having a
/// 2 entries (an array) of 4x4 matrices (with 16 components each).
/// As a packed linear buffer, this would occupy 10*2*16==320 floats.
///
/// It is important to distinguish components from array entries,
/// and arrays from elements. HdType and HdTupleType only
/// address components and arrays; elements are tracked by buffers.
///
/// In other words, HdType and HdTupleType describe values.
/// Buffers describe elements and all other details regarding buffer
/// layout, such as offset/stride used to interleave attribute data.
///
/// For more background, see the OpenGL discussion on data types:
/// <https://www.khronos.org/opengl/wiki/OpenGL_Type>
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HdType {
    /// Invalid or unknown type.
    Invalid = -1,

    /// Boolean type. Corresponds to `GL_BOOL`.
    Bool = 0,
    /// Unsigned 8-bit integer.
    UInt8,
    /// Unsigned 16-bit integer.
    UInt16,
    /// Signed 8-bit integer.
    Int8,
    /// Signed 16-bit integer.
    Int16,

    /// Signed 32-bit integer. Corresponds to `GL_INT`.
    Int32,
    /// A 2-component vector with Int32-valued components.
    Int32Vec2,
    /// A 3-component vector with Int32-valued components.
    Int32Vec3,
    /// A 4-component vector with Int32-valued components.
    Int32Vec4,

    /// An unsigned 32-bit integer. Corresponds to `GL_UNSIGNED_INT`.
    UInt32,
    /// A 2-component vector with UInt32-valued components.
    UInt32Vec2,
    /// A 3-component vector with UInt32-valued components.
    UInt32Vec3,
    /// A 4-component vector with UInt32-valued components.
    UInt32Vec4,

    /// Single-precision float. Corresponds to `GL_FLOAT`.
    Float,
    /// 2-component float vector. Corresponds to `GL_FLOAT_VEC2`.
    FloatVec2,
    /// 3-component float vector. Corresponds to `GL_FLOAT_VEC3`.
    FloatVec3,
    /// 4-component float vector. Corresponds to `GL_FLOAT_VEC4`.
    FloatVec4,
    /// 3x3 float matrix. Corresponds to `GL_FLOAT_MAT3`.
    FloatMat3,
    /// 4x4 float matrix. Corresponds to `GL_FLOAT_MAT4`.
    FloatMat4,

    /// Double-precision float. Corresponds to `GL_DOUBLE`.
    /// NOTE: Order must match C++ HdType (Double before HalfFloat).
    Double,
    /// 2-component double vector. Corresponds to `GL_DOUBLE_VEC2`.
    DoubleVec2,
    /// 3-component double vector. Corresponds to `GL_DOUBLE_VEC3`.
    DoubleVec3,
    /// 4-component double vector. Corresponds to `GL_DOUBLE_VEC4`.
    DoubleVec4,
    /// 3x3 double matrix. Corresponds to `GL_DOUBLE_MAT3`.
    DoubleMat3,
    /// 4x4 double matrix. Corresponds to `GL_DOUBLE_MAT4`.
    DoubleMat4,

    /// Half-precision (16-bit) float.
    HalfFloat,
    /// 2-component half-float vector.
    HalfFloatVec2,
    /// 3-component half-float vector.
    HalfFloatVec3,
    /// 4-component half-float vector.
    HalfFloatVec4,

    /// Packed, reverse-order encoding of a 4-component vector into Int32.
    ///
    /// Corresponds to `GL_INT_2_10_10_10_REV`.
    /// See [`HdVec4_2_10_10_10_Rev`] for the packed struct representation.
    #[allow(non_camel_case_types)]
    Int32_2_10_10_10_Rev,
}

/// Number of HdType enum values. Matches C++ `HdTypeCount`.
/// Use for array sizing when indexing by type discriminant.
pub const HD_TYPE_COUNT: usize = 30;

impl HdType {
    /// Returns the count of components in this type.
    ///
    /// For example:
    /// - Scalars return 1
    /// - `FloatVec3` returns 3
    /// - `FloatMat4` returns 16 (4x4 matrix)
    ///
    /// # Returns
    ///
    /// Number of scalar components in this type.
    pub fn component_count(self) -> usize {
        match self {
            Self::Invalid => 0,
            Self::Bool
            | Self::UInt8
            | Self::UInt16
            | Self::Int8
            | Self::Int16
            | Self::Int32
            | Self::UInt32
            | Self::Float
            | Self::HalfFloat
            | Self::Double => 1,
            Self::Int32Vec2
            | Self::UInt32Vec2
            | Self::FloatVec2
            | Self::HalfFloatVec2
            | Self::DoubleVec2 => 2,
            Self::Int32Vec3
            | Self::UInt32Vec3
            | Self::FloatVec3
            | Self::HalfFloatVec3
            | Self::DoubleVec3 => 3,
            Self::Int32Vec4
            | Self::UInt32Vec4
            | Self::FloatVec4
            | Self::HalfFloatVec4
            | Self::DoubleVec4
            | Self::Int32_2_10_10_10_Rev => 4,
            Self::FloatMat3 | Self::DoubleMat3 => 9,
            Self::FloatMat4 | Self::DoubleMat4 => 16,
        }
    }

    /// Returns the component base type for this type.
    ///
    /// For vectors and matrices, this returns the scalar type of their components.
    /// For scalars, this returns the type itself.
    ///
    /// # Examples
    ///
    /// - `FloatVec3.component_type()` returns `Float`
    /// - `FloatMat4.component_type()` returns `Float`
    /// - `Float.component_type()` returns `Float`
    ///
    /// # Returns
    ///
    /// The scalar component type.
    pub fn component_type(self) -> Self {
        match self {
            Self::Int32Vec2 | Self::Int32Vec3 | Self::Int32Vec4 => Self::Int32,
            Self::UInt32Vec2 | Self::UInt32Vec3 | Self::UInt32Vec4 => Self::UInt32,
            Self::FloatVec2
            | Self::FloatVec3
            | Self::FloatVec4
            | Self::FloatMat3
            | Self::FloatMat4 => Self::Float,
            Self::HalfFloatVec2 | Self::HalfFloatVec3 | Self::HalfFloatVec4 => Self::HalfFloat,
            Self::DoubleVec2
            | Self::DoubleVec3
            | Self::DoubleVec4
            | Self::DoubleMat3
            | Self::DoubleMat4 => Self::Double,
            _ => self,
        }
    }

    /// Returns the size in bytes for a single value of this type.
    ///
    /// # Examples
    ///
    /// - `Float.size_in_bytes()` returns 4
    /// - `FloatVec3.size_in_bytes()` returns 12
    /// - `FloatMat4.size_in_bytes()` returns 64
    ///
    /// # Returns
    ///
    /// Size in bytes of a single value of this type.
    pub fn size_in_bytes(self) -> usize {
        match self {
            Self::Invalid => 0,
            // XXX: Hd represents bools as int32 sized values for GPU buffer alignment.
            Self::Bool => 4,
            Self::UInt8 | Self::Int8 => 1,
            Self::UInt16 | Self::Int16 | Self::HalfFloat => 2,
            Self::Int32 | Self::UInt32 | Self::Float | Self::Int32_2_10_10_10_Rev => 4,
            Self::Double => 8,
            Self::Int32Vec2 | Self::UInt32Vec2 | Self::FloatVec2 | Self::HalfFloatVec2 => {
                self.component_type().size_in_bytes() * 2
            }
            Self::Int32Vec3 | Self::UInt32Vec3 | Self::FloatVec3 | Self::HalfFloatVec3 => {
                self.component_type().size_in_bytes() * 3
            }
            Self::Int32Vec4 | Self::UInt32Vec4 | Self::FloatVec4 | Self::HalfFloatVec4 => {
                self.component_type().size_in_bytes() * 4
            }
            Self::DoubleVec2 => 16,
            Self::DoubleVec3 => 24,
            Self::DoubleVec4 => 32,
            Self::FloatMat3 => 36,
            Self::DoubleMat3 => 72,
            Self::FloatMat4 => 64,
            Self::DoubleMat4 => 128,
        }
    }
}

/// Represents zero, one, or more values of the same HdType.
///
/// HdTupleType combines an [`HdType`] with a count to represent fixed-size
/// array types, as well as single values. This is used anywhere there is a
/// possibility of an array of values.
///
/// See [`HdType`] for more discussion about arrays, components, and elements.
///
/// # Examples
///
/// ```ignore
/// // Single float value
/// let single = HdTupleType::single(HdType::Float);
///
/// // Array of 4 vec3 values
/// let array = HdTupleType::new(HdType::FloatVec3, 4);
/// assert_eq!(array.size_in_bytes(), 48); // 12 bytes * 4
/// ```
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct HdTupleType {
    /// The base type of values in this tuple.
    pub type_: HdType,
    /// The number of values in this tuple (array size).
    pub count: usize,
}

impl HdTupleType {
    /// Creates a new tuple type with the specified base type and count.
    ///
    /// # Arguments
    ///
    /// * `type_` - The base [`HdType`] for values in this tuple
    /// * `count` - The number of values (array size)
    ///
    /// # Returns
    ///
    /// A new `HdTupleType` instance.
    pub fn new(type_: HdType, count: usize) -> Self {
        Self { type_, count }
    }

    /// Creates a tuple type for a single value (count = 1).
    ///
    /// # Arguments
    ///
    /// * `type_` - The [`HdType`] for the single value
    ///
    /// # Returns
    ///
    /// A new `HdTupleType` with count set to 1.
    pub fn single(type_: HdType) -> Self {
        Self { type_, count: 1 }
    }

    /// Returns the total size in bytes for this tuple.
    ///
    /// This is calculated as `type.size_in_bytes() * count`.
    ///
    /// # Returns
    ///
    /// Total size in bytes for all values in this tuple.
    pub fn size_in_bytes(&self) -> usize {
        self.type_.size_in_bytes() * self.count
    }

    /// Returns the total component count across all values in this tuple.
    ///
    /// This is calculated as `type.component_count() * count`.
    ///
    /// # Returns
    ///
    /// Total number of scalar components in this tuple.
    pub fn total_components(&self) -> usize {
        self.type_.component_count() * self.count
    }
}

impl Default for HdTupleType {
    fn default() -> Self {
        Self {
            type_: HdType::Invalid,
            count: 0,
        }
    }
}

impl PartialOrd for HdTupleType {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for HdTupleType {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        match (self.type_ as i32).cmp(&(other.type_ as i32)) {
            std::cmp::Ordering::Equal => self.count.cmp(&other.count),
            ord => ord,
        }
    }
}

/// Collection of standard parameters used to sample a texture.
///
/// `HdSamplerParameters` contains all the settings that control how a texture
/// is sampled, including wrap modes for each axis, minification and magnification
/// filters, border colors, comparison functions for depth textures, and anisotropic
/// filtering settings.
///
/// These parameters correspond to standard GPU texture sampler state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HdSamplerParameters {
    /// Wrap mode for S texture coordinate (horizontal).
    pub wrap_s: HdWrap,
    /// Wrap mode for T texture coordinate (vertical).
    pub wrap_t: HdWrap,
    /// Wrap mode for R texture coordinate (depth for 3D textures).
    pub wrap_r: HdWrap,
    /// Minification filter (when texture is smaller than screen pixels).
    pub min_filter: HdMinFilter,
    /// Magnification filter (when texture is larger than screen pixels).
    pub mag_filter: HdMagFilter,
    /// Border color to use when wrap mode is set to clamp to border.
    pub border_color: HdBorderColor,
    /// Enable depth comparison for shadow mapping.
    pub enable_compare: bool,
    /// Comparison function to use when enable_compare is true.
    pub compare_function: HdCompareFunction,
    /// Maximum anisotropy level (1-16, higher values improve quality at oblique angles).
    pub max_anisotropy: u32,
}

impl Default for HdSamplerParameters {
    /// C++ default: HdWrapRepeat, HdWrapRepeat, HdWrapClamp,
    /// HdMinFilterNearest, HdMagFilterNearest
    fn default() -> Self {
        Self {
            wrap_s: HdWrap::Repeat,
            wrap_t: HdWrap::Repeat,
            wrap_r: HdWrap::Clamp,
            min_filter: HdMinFilter::Nearest,
            mag_filter: HdMagFilter::Nearest,
            border_color: HdBorderColor::TransparentBlack,
            enable_compare: false,
            compare_function: HdCompareFunction::Never,
            max_anisotropy: 16,
        }
    }
}

impl HdSamplerParameters {
    /// Creates sampler parameters with explicit values for all settings.
    ///
    /// # Arguments
    ///
    /// * `wrap_s` - Wrap mode for S coordinate
    /// * `wrap_t` - Wrap mode for T coordinate
    /// * `wrap_r` - Wrap mode for R coordinate
    /// * `min_filter` - Minification filter
    /// * `mag_filter` - Magnification filter
    /// * `border_color` - Border color for clamped textures
    /// * `enable_compare` - Enable depth comparison
    /// * `compare_function` - Comparison function for depth textures
    /// * `max_anisotropy` - Maximum anisotropy level (1-16)
    pub fn new(
        wrap_s: HdWrap,
        wrap_t: HdWrap,
        wrap_r: HdWrap,
        min_filter: HdMinFilter,
        mag_filter: HdMagFilter,
        border_color: HdBorderColor,
        enable_compare: bool,
        compare_function: HdCompareFunction,
        max_anisotropy: u32,
    ) -> Self {
        Self {
            wrap_s,
            wrap_t,
            wrap_r,
            min_filter,
            mag_filter,
            border_color,
            enable_compare,
            compare_function,
            max_anisotropy,
        }
    }
}

// -----------------------------------------------------------------------//
// HdFormat - Image buffer memory format (from hd/types.h)
// -----------------------------------------------------------------------//

/// Describes the memory format of image buffers used in Hydra.
///
/// Similar to HdType but with more specific associated semantics.
/// The list of supported formats is modelled after Vulkan and DXGI,
/// though Hydra only supports a subset. Endian-ness is explicitly not
/// captured; color data is assumed to always be RGBA.
///
/// Corresponds to C++ `HdFormat` in pxr/imaging/hd/types.h.
#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HdFormat {
    /// Invalid or unknown format.
    Invalid = -1,

    /// UNorm8: 1-byte value representing a float in [0, 1].
    UNorm8 = 0,
    /// 2-component UNorm8 vector.
    UNorm8Vec2,
    /// 3-component UNorm8 vector.
    UNorm8Vec3,
    /// 4-component UNorm8 vector.
    UNorm8Vec4,

    /// SNorm8: 1-byte value representing a float in [-1, 1].
    SNorm8,
    /// 2-component SNorm8 vector.
    SNorm8Vec2,
    /// 3-component SNorm8 vector.
    SNorm8Vec3,
    /// 4-component SNorm8 vector.
    SNorm8Vec4,

    /// Float16: 2-byte IEEE half-precision float.
    Float16,
    /// 2-component Float16 vector.
    Float16Vec2,
    /// 3-component Float16 vector.
    Float16Vec3,
    /// 4-component Float16 vector.
    Float16Vec4,

    /// Float32: 4-byte IEEE single-precision float.
    Float32,
    /// 2-component Float32 vector.
    Float32Vec2,
    /// 3-component Float32 vector.
    Float32Vec3,
    /// 4-component Float32 vector.
    Float32Vec4,

    /// Int16: 2-byte signed integer.
    Int16,
    /// 2-component Int16 vector.
    Int16Vec2,
    /// 3-component Int16 vector.
    Int16Vec3,
    /// 4-component Int16 vector.
    Int16Vec4,

    /// UInt16: 2-byte unsigned integer.
    UInt16,
    /// 2-component UInt16 vector.
    UInt16Vec2,
    /// 3-component UInt16 vector.
    UInt16Vec3,
    /// 4-component UInt16 vector.
    UInt16Vec4,

    /// Int32: 4-byte signed integer.
    Int32,
    /// 2-component Int32 vector.
    Int32Vec2,
    /// 3-component Int32 vector.
    Int32Vec3,
    /// 4-component Int32 vector.
    Int32Vec4,

    /// Depth-stencil combined format (float32 depth + uint8 stencil).
    Float32UInt8,
}

/// Number of HdFormat enum values. Matches C++ `HdFormatCount`.
pub const HD_FORMAT_COUNT: usize = 29;

impl Default for HdFormat {
    fn default() -> Self {
        HdFormat::Invalid
    }
}

impl HdFormat {
    /// Returns the single-channel version of this format.
    ///
    /// Corresponds to C++ `HdGetComponentFormat()`.
    pub fn component_format(self) -> Self {
        match self {
            Self::UNorm8 | Self::UNorm8Vec2 | Self::UNorm8Vec3 | Self::UNorm8Vec4 => Self::UNorm8,
            Self::SNorm8 | Self::SNorm8Vec2 | Self::SNorm8Vec3 | Self::SNorm8Vec4 => Self::SNorm8,
            Self::Float16 | Self::Float16Vec2 | Self::Float16Vec3 | Self::Float16Vec4 => {
                Self::Float16
            }
            Self::Float32 | Self::Float32Vec2 | Self::Float32Vec3 | Self::Float32Vec4 => {
                Self::Float32
            }
            Self::Int16 | Self::Int16Vec2 | Self::Int16Vec3 | Self::Int16Vec4 => Self::Int16,
            Self::UInt16 | Self::UInt16Vec2 | Self::UInt16Vec3 | Self::UInt16Vec4 => Self::UInt16,
            Self::Int32 | Self::Int32Vec2 | Self::Int32Vec3 | Self::Int32Vec4 => Self::Int32,
            Self::Float32UInt8 => Self::Float32UInt8, // treat as a single component
            _ => Self::Invalid,
        }
    }

    /// Returns the number of components in this format.
    ///
    /// Corresponds to C++ `HdGetComponentCount(HdFormat)`.
    pub fn component_count(self) -> usize {
        match self {
            Self::UNorm8Vec2
            | Self::SNorm8Vec2
            | Self::Float16Vec2
            | Self::Float32Vec2
            | Self::Int16Vec2
            | Self::UInt16Vec2
            | Self::Int32Vec2 => 2,
            Self::UNorm8Vec3
            | Self::SNorm8Vec3
            | Self::Float16Vec3
            | Self::Float32Vec3
            | Self::Int16Vec3
            | Self::UInt16Vec3
            | Self::Int32Vec3 => 3,
            Self::UNorm8Vec4
            | Self::SNorm8Vec4
            | Self::Float16Vec4
            | Self::Float32Vec4
            | Self::Int16Vec4
            | Self::UInt16Vec4
            | Self::Int32Vec4 => 4,
            _ => 1,
        }
    }

    /// Returns the size in bytes of a single element of this format.
    ///
    /// Returns 0 for block formats and Invalid.
    /// Corresponds to C++ `HdDataSizeOfFormat()`.
    pub fn size_in_bytes(self) -> usize {
        match self {
            Self::UNorm8 | Self::SNorm8 => 1,
            Self::UNorm8Vec2 | Self::SNorm8Vec2 => 2,
            Self::UNorm8Vec3 | Self::SNorm8Vec3 => 3,
            Self::UNorm8Vec4 | Self::SNorm8Vec4 => 4,
            Self::Float16 | Self::Int16 | Self::UInt16 => 2,
            Self::Float16Vec2 | Self::Int16Vec2 | Self::UInt16Vec2 => 4,
            Self::Float16Vec3 | Self::Int16Vec3 | Self::UInt16Vec3 => 6,
            Self::Float16Vec4 | Self::Int16Vec4 | Self::UInt16Vec4 => 8,
            Self::Float32 | Self::Int32 => 4,
            Self::Float32Vec2 | Self::Int32Vec2 | Self::Float32UInt8 => 8, // XXX: implementation dependent
            Self::Float32Vec3 | Self::Int32Vec3 => 12,
            Self::Float32Vec4 | Self::Int32Vec4 => 16,
            _ => 0,
        }
    }
}

/// Depth-stencil value type (float depth + u32 stencil).
pub type HdDepthStencilType = (f32, u32);

/// Returns a raw pointer to the data held by a Value.
///
/// Maps known Gf types (Vec3f, Matrix4d, f32, etc.) to their raw data pointers.
/// Returns None for unsupported or empty values.
///
/// Corresponds to C++ `HdGetValueData(VtValue)`.
pub fn hd_get_value_data(value: &Value) -> Option<*const u8> {
    // Macro to reduce boilerplate: try downcast, return data ptr
    macro_rules! try_type {
        ($T:ty) => {
            if let Some(v) = value.get::<$T>() {
                return Some(v as *const $T as *const u8);
            }
        };
    }

    if value.is_empty() {
        return None;
    }

    // Scalar types
    try_type!(f32);
    try_type!(f64);
    try_type!(bool);
    try_type!(i8);
    try_type!(i16);
    try_type!(i32);
    try_type!(u8);
    try_type!(u16);
    try_type!(u32);

    // Vector types
    try_type!(Vec2f);
    try_type!(Vec2d);
    try_type!(Vec2i);
    try_type!(Vec3f);
    try_type!(Vec3d);
    try_type!(Vec3i);
    try_type!(Vec4f);
    try_type!(Vec4d);
    try_type!(Vec4i);

    // Matrix types
    try_type!(Matrix3f);
    try_type!(Matrix3d);
    try_type!(Matrix4f);
    try_type!(Matrix4d);

    // Quaternion types (mapped to vec4 storage)
    try_type!(Quatf);
    try_type!(Quatd);
    try_type!(Quath);

    // Packed type
    try_type!(HdVec4_2_10_10_10_Rev);

    None
}

/// Maps a scalar type to its corresponding HdType.
///
/// Used internally by `hd_get_value_tuple_type`.
fn value_type_to_hd_type(value: &Value) -> Option<HdType> {
    // Macro: check scalar type
    macro_rules! check {
        ($T:ty, $hd:expr) => {
            if value.is::<$T>() {
                return Some($hd);
            }
        };
    }

    check!(f32, HdType::Float);
    check!(f64, HdType::Double);
    check!(bool, HdType::Bool);
    check!(i8, HdType::Int8);
    check!(i16, HdType::Int16);
    check!(i32, HdType::Int32);
    check!(u8, HdType::UInt8);
    check!(u16, HdType::UInt16);
    check!(u32, HdType::UInt32);
    check!(Vec2f, HdType::FloatVec2);
    check!(Vec2d, HdType::DoubleVec2);
    check!(Vec2i, HdType::Int32Vec2);
    check!(Vec3f, HdType::FloatVec3);
    check!(Vec3d, HdType::DoubleVec3);
    check!(Vec3i, HdType::Int32Vec3);
    check!(Vec4f, HdType::FloatVec4);
    check!(Vec4d, HdType::DoubleVec4);
    check!(Vec4i, HdType::Int32Vec4);
    check!(Matrix3f, HdType::FloatMat3);
    check!(Matrix3d, HdType::DoubleMat3);
    check!(Matrix4f, HdType::FloatMat4);
    check!(Matrix4d, HdType::DoubleMat4);
    check!(Quatf, HdType::FloatVec4);
    check!(Quatd, HdType::DoubleVec4);
    check!(Quath, HdType::HalfFloatVec4);
    check!(HdVec4_2_10_10_10_Rev, HdType::Int32_2_10_10_10_Rev);

    None
}

/// Maps a VtArray element type to HdType for array-valued Values.
fn array_element_type_to_hd_type(value: &Value) -> Option<HdType> {
    use usd_vt::Array;

    macro_rules! check_arr {
        ($T:ty, $hd:expr) => {
            if value.is::<Array<$T>>() {
                return Some($hd);
            }
        };
    }

    check_arr!(f32, HdType::Float);
    check_arr!(f64, HdType::Double);
    check_arr!(i32, HdType::Int32);
    check_arr!(u32, HdType::UInt32);
    check_arr!(bool, HdType::Bool);
    check_arr!(i8, HdType::Int8);
    check_arr!(i16, HdType::Int16);
    check_arr!(u8, HdType::UInt8);
    check_arr!(u16, HdType::UInt16);
    check_arr!(Vec2f, HdType::FloatVec2);
    check_arr!(Vec2d, HdType::DoubleVec2);
    check_arr!(Vec2i, HdType::Int32Vec2);
    check_arr!(Vec3f, HdType::FloatVec3);
    check_arr!(Vec3d, HdType::DoubleVec3);
    check_arr!(Vec3i, HdType::Int32Vec3);
    check_arr!(Vec4f, HdType::FloatVec4);
    check_arr!(Vec4d, HdType::DoubleVec4);
    check_arr!(Vec4i, HdType::Int32Vec4);
    check_arr!(Matrix3f, HdType::FloatMat3);
    check_arr!(Matrix3d, HdType::DoubleMat3);
    check_arr!(Matrix4f, HdType::FloatMat4);
    check_arr!(Matrix4d, HdType::DoubleMat4);

    None
}

/// Returns the HdTupleType for a Value.
///
/// For scalar values, returns (type, 1). For arrays, returns (element_type, array_size).
/// For unsupported types, returns (Invalid, 0).
///
/// Corresponds to C++ `HdGetValueTupleType(VtValue)`.
pub fn hd_get_value_tuple_type(value: &Value) -> HdTupleType {
    if value.is_empty() {
        return HdTupleType::default();
    }

    if value.is_array_valued() {
        // Array: element type + count
        match array_element_type_to_hd_type(value) {
            Some(hd_type) => HdTupleType::new(hd_type, value.array_size()),
            None => HdTupleType::default(),
        }
    } else {
        // Scalar: type + count=1
        match value_type_to_hd_type(value) {
            Some(hd_type) => HdTupleType::new(hd_type, 1),
            None => HdTupleType::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hd_type_sizes() {
        assert_eq!(HdType::Float.size_in_bytes(), 4);
        assert_eq!(HdType::FloatVec3.size_in_bytes(), 12);
        assert_eq!(HdType::FloatMat4.size_in_bytes(), 64);
        assert_eq!(HdType::Double.size_in_bytes(), 8);
        assert_eq!(HdType::DoubleMat4.size_in_bytes(), 128);
        // Bool is represented as int32 for GPU buffer alignment
        assert_eq!(HdType::Bool.size_in_bytes(), 4);
    }

    #[test]
    fn test_hd_type_components() {
        assert_eq!(HdType::Float.component_count(), 1);
        assert_eq!(HdType::FloatVec3.component_count(), 3);
        assert_eq!(HdType::FloatMat4.component_count(), 16);
    }

    #[test]
    fn test_hd_type_component_type() {
        assert_eq!(HdType::FloatVec3.component_type(), HdType::Float);
        assert_eq!(HdType::FloatMat4.component_type(), HdType::Float);
        assert_eq!(HdType::DoubleVec4.component_type(), HdType::Double);
    }

    #[test]
    fn test_tuple_type() {
        let tuple = HdTupleType::new(HdType::FloatVec3, 4);
        assert_eq!(tuple.size_in_bytes(), 48); // 12 bytes * 4
        assert_eq!(tuple.total_components(), 12); // 3 components * 4

        let single = HdTupleType::single(HdType::Float);
        assert_eq!(single.count, 1);
        assert_eq!(single.size_in_bytes(), 4);
    }

    #[test]
    fn test_tuple_type_ordering() {
        let t1 = HdTupleType::new(HdType::Float, 1);
        let t2 = HdTupleType::new(HdType::Float, 2);
        let t3 = HdTupleType::new(HdType::Double, 1);

        assert!(t1 < t2);
        assert!(t1 < t3);
    }

    #[test]
    fn test_sampler_parameters_default() {
        // C++ default: Repeat/Repeat/Clamp, Nearest/Nearest
        let params = HdSamplerParameters::default();
        assert_eq!(params.wrap_s, HdWrap::Repeat);
        assert_eq!(params.wrap_t, HdWrap::Repeat);
        assert_eq!(params.wrap_r, HdWrap::Clamp);
        assert_eq!(params.min_filter, HdMinFilter::Nearest);
        assert_eq!(params.mag_filter, HdMagFilter::Nearest);
        assert_eq!(params.max_anisotropy, 16);
    }

    #[test]
    fn test_vec4_2_10_10_10_rev() {
        let vec = HdVec4_2_10_10_10_Rev::from_vec3(0.5, -0.5, 1.0);
        let (x, y, z) = vec.to_vec3();

        // Allow some tolerance for fixed-point conversion
        assert!((x - 0.5).abs() < 0.01);
        assert!((y + 0.5).abs() < 0.01);
        assert!((z - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_fixed_conversion() {
        // Test clamping
        assert_eq!(convert_float_to_fixed(2.0, 10), 511); // Max value
        assert_eq!(convert_float_to_fixed(-2.0, 10), -511); // Min value

        // Test round-trip
        let fixed = convert_float_to_fixed(0.5, 10);
        let float = convert_fixed_to_float(fixed, 10);
        assert!((float - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_hd_format_component_format() {
        assert_eq!(HdFormat::UNorm8Vec3.component_format(), HdFormat::UNorm8);
        assert_eq!(HdFormat::Float32Vec4.component_format(), HdFormat::Float32);
        assert_eq!(HdFormat::Int32Vec2.component_format(), HdFormat::Int32);
        assert_eq!(
            HdFormat::Float32UInt8.component_format(),
            HdFormat::Float32UInt8
        );
        assert_eq!(HdFormat::Invalid.component_format(), HdFormat::Invalid);
    }

    #[test]
    fn test_hd_format_component_count() {
        assert_eq!(HdFormat::UNorm8.component_count(), 1);
        assert_eq!(HdFormat::Float32Vec2.component_count(), 2);
        assert_eq!(HdFormat::SNorm8Vec3.component_count(), 3);
        assert_eq!(HdFormat::Float16Vec4.component_count(), 4);
    }

    #[test]
    fn test_hd_format_size_in_bytes() {
        assert_eq!(HdFormat::UNorm8.size_in_bytes(), 1);
        assert_eq!(HdFormat::Float16.size_in_bytes(), 2);
        assert_eq!(HdFormat::Float32.size_in_bytes(), 4);
        assert_eq!(HdFormat::Float32Vec4.size_in_bytes(), 16);
        assert_eq!(HdFormat::Int32Vec3.size_in_bytes(), 12);
        assert_eq!(HdFormat::Float32UInt8.size_in_bytes(), 8);
        assert_eq!(HdFormat::Invalid.size_in_bytes(), 0);
    }

    #[test]
    fn test_hd_type_count() {
        // HdTypeCount should be 30 matching C++
        assert_eq!(HD_TYPE_COUNT, 30);
        // Verify last valid enum discriminant is 29 (Int32_2_10_10_10_Rev)
        assert_eq!(HdType::Int32_2_10_10_10_Rev as i32, 29);
    }

    #[test]
    fn test_hd_get_value_data_scalars() {
        let v = Value::from(42i32);
        assert!(hd_get_value_data(&v).is_some());
        let v = Value::from(3.14f32);
        assert!(hd_get_value_data(&v).is_some());
        let v = Value::empty();
        assert!(hd_get_value_data(&v).is_none());
    }

    #[test]
    fn test_hd_get_value_data_vectors() {
        let v = Value::from(Vec3f::new(1.0, 2.0, 3.0));
        let ptr = hd_get_value_data(&v);
        assert!(ptr.is_some());
    }

    #[test]
    fn test_hd_get_value_tuple_type_scalar() {
        let tt = hd_get_value_tuple_type(&Value::from_f32(1.0f32));
        assert_eq!(tt.type_, HdType::Float);
        assert_eq!(tt.count, 1);

        let tt = hd_get_value_tuple_type(&Value::from(42i32));
        assert_eq!(tt.type_, HdType::Int32);
        assert_eq!(tt.count, 1);

        let tt = hd_get_value_tuple_type(&Value::from(true));
        assert_eq!(tt.type_, HdType::Bool);
        assert_eq!(tt.count, 1);
    }

    #[test]
    fn test_hd_get_value_tuple_type_array() {
        use usd_vt::Array;
        // Vec3f contains f32 which doesn't impl Hash, use from_no_hash
        let arr = Array::from(vec![Vec3f::new(1.0, 2.0, 3.0), Vec3f::new(4.0, 5.0, 6.0)]);
        let tt = hd_get_value_tuple_type(&Value::from_no_hash(arr));
        assert_eq!(tt.type_, HdType::FloatVec3);
        assert_eq!(tt.count, 2);
    }

    #[test]
    fn test_hd_get_value_tuple_type_empty() {
        let tt = hd_get_value_tuple_type(&Value::empty());
        assert_eq!(tt.type_, HdType::Invalid);
        assert_eq!(tt.count, 0);
    }
}

// Value::from for HdTupleType (used by HdRetainedTypedSampledDataSource in ext computation primvar)
impl From<HdTupleType> for usd_vt::Value {
    #[inline]
    fn from(value: HdTupleType) -> Self {
        Self::new(value)
    }
}
