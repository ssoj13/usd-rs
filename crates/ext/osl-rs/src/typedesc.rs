//! Binary-compatible type descriptor matching OIIO `TypeDesc`.
//!
//! `TypeDesc` is an 8-byte packed struct that encodes base type, aggregate,
//! vector semantics, and array length. Its in-memory layout matches
//! `TypeDesc_pod = i64` used in the C++ OSL/OIIO libraries.

use std::fmt;

/// Base data types, matching OIIO `TypeDesc::BASETYPE`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum BaseType {
    Unknown = 0,
    None = 1,
    UInt8 = 2,
    Int8 = 3,
    UInt16 = 4,
    Int16 = 5,
    UInt32 = 6,
    Int32 = 7,
    UInt64 = 8,
    Int64 = 9,
    Half = 10,
    Float = 11,
    Double = 12,
    String = 13,
    Ptr = 14,
    UStringHash = 15,
}

impl BaseType {
    /// Size in bytes of a single element of this base type.
    pub const fn size(self) -> usize {
        match self {
            BaseType::Unknown | BaseType::None => 0,
            BaseType::UInt8 | BaseType::Int8 => 1,
            BaseType::UInt16 | BaseType::Int16 => 2,
            BaseType::UInt32 | BaseType::Int32 => 4,
            BaseType::UInt64 | BaseType::Int64 => 8,
            BaseType::Half => 2,
            BaseType::Float => 4,
            BaseType::Double => 8,
            BaseType::String => std::mem::size_of::<usize>(),
            BaseType::Ptr => std::mem::size_of::<usize>(),
            BaseType::UStringHash => std::mem::size_of::<usize>(),
        }
    }

    /// Convert from raw u8 value. Returns `Unknown` for invalid values.
    pub const fn from_u8(v: u8) -> Self {
        match v {
            0 => BaseType::Unknown,
            1 => BaseType::None,
            2 => BaseType::UInt8,
            3 => BaseType::Int8,
            4 => BaseType::UInt16,
            5 => BaseType::Int16,
            6 => BaseType::UInt32,
            7 => BaseType::Int32,
            8 => BaseType::UInt64,
            9 => BaseType::Int64,
            10 => BaseType::Half,
            11 => BaseType::Float,
            12 => BaseType::Double,
            13 => BaseType::String,
            14 => BaseType::Ptr,
            15 => BaseType::UStringHash,
            _ => BaseType::Unknown,
        }
    }
}

/// Aggregation of base types, matching OIIO `TypeDesc::AGGREGATE`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Aggregate {
    Scalar = 1,
    Vec2 = 2,
    Vec3 = 3,
    Vec4 = 4,
    Matrix33 = 9,
    Matrix44 = 16,
}

impl Aggregate {
    /// Number of scalar components in this aggregate.
    pub const fn count(self) -> usize {
        match self {
            Aggregate::Scalar => 1,
            Aggregate::Vec2 => 2,
            Aggregate::Vec3 => 3,
            Aggregate::Vec4 => 4,
            Aggregate::Matrix33 => 9,
            Aggregate::Matrix44 => 16,
        }
    }

    pub const fn from_u8(v: u8) -> Self {
        match v {
            2 => Aggregate::Vec2,
            3 => Aggregate::Vec3,
            4 => Aggregate::Vec4,
            9 => Aggregate::Matrix33,
            16 => Aggregate::Matrix44,
            _ => Aggregate::Scalar,
        }
    }
}

/// Vector semantics, matching OIIO `TypeDesc::VECSEMANTICS`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum VecSemantics {
    NoXform = 0,
    Color = 1,
    Point = 2,
    Vector = 3,
    Normal = 4,
    Timecode = 10,
    Keycode = 11,
    Rational = 12,
}

impl VecSemantics {
    pub const fn from_u8(v: u8) -> Self {
        match v {
            1 => VecSemantics::Color,
            2 => VecSemantics::Point,
            3 => VecSemantics::Vector,
            4 => VecSemantics::Normal,
            10 => VecSemantics::Timecode,
            11 => VecSemantics::Keycode,
            12 => VecSemantics::Rational,
            _ => VecSemantics::NoXform,
        }
    }
}

/// A compact type descriptor, binary-compatible with OIIO `TypeDesc`.
///
/// Layout (8 bytes total, matches `TypeDesc_pod = i64`):
/// ```text
/// byte 0: basetype      (BaseType)
/// byte 1: aggregate     (Aggregate)
/// byte 2: vecsemantics  (VecSemantics)
/// byte 3: reserved      (always 0)
/// bytes 4-7: arraylen   (i32, 0 = not array, -1 = unsized)
/// ```
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
#[repr(C)]
pub struct TypeDesc {
    pub basetype: u8,
    pub aggregate: u8,
    pub vecsemantics: u8,
    pub reserved: u8,
    pub arraylen: i32,
}

// Ensure binary compatibility: TypeDesc must be exactly 8 bytes = i64.
const _: () = assert!(std::mem::size_of::<TypeDesc>() == 8);
const _: () = assert!(std::mem::align_of::<TypeDesc>() == 4);

/// POD representation used in LLVM function calls.
pub type TypeDescPod = i64;

impl TypeDesc {
    /// Create a simple scalar type.
    pub const fn scalar(basetype: BaseType) -> Self {
        Self {
            basetype: basetype as u8,
            aggregate: Aggregate::Scalar as u8,
            vecsemantics: VecSemantics::NoXform as u8,
            reserved: 0,
            arraylen: 0,
        }
    }

    /// Create a type with specified base, aggregate, and semantics.
    pub const fn new(basetype: BaseType, aggregate: Aggregate, vecsemantics: VecSemantics) -> Self {
        Self {
            basetype: basetype as u8,
            aggregate: aggregate as u8,
            vecsemantics: vecsemantics as u8,
            reserved: 0,
            arraylen: 0,
        }
    }

    /// Create an array type from this type.
    pub const fn array(mut self, len: i32) -> Self {
        self.arraylen = len;
        self
    }

    /// Unknown type.
    pub const UNKNOWN: Self = Self::scalar(BaseType::Unknown);
    /// No type / void.
    pub const NONE: Self = Self::scalar(BaseType::None);
    /// Single float.
    pub const FLOAT: Self = Self::scalar(BaseType::Float);
    /// Single int (i32).
    pub const INT: Self = Self::scalar(BaseType::Int32);
    /// Single string.
    pub const STRING: Self = Self::scalar(BaseType::String);
    /// Single pointer.
    pub const PTR: Self = Self::scalar(BaseType::Ptr);

    /// Color (float×3, color semantics).
    pub const COLOR: Self = Self::new(BaseType::Float, Aggregate::Vec3, VecSemantics::Color);
    /// Point (float×3, point semantics).
    pub const POINT: Self = Self::new(BaseType::Float, Aggregate::Vec3, VecSemantics::Point);
    /// Vector (float×3, vector semantics).
    pub const VECTOR: Self = Self::new(BaseType::Float, Aggregate::Vec3, VecSemantics::Vector);
    /// Normal (float×3, normal semantics).
    pub const NORMAL: Self = Self::new(BaseType::Float, Aggregate::Vec3, VecSemantics::Normal);
    /// 4×4 matrix of float.
    pub const MATRIX: Self = Self::new(BaseType::Float, Aggregate::Matrix44, VecSemantics::NoXform);
    /// 3×3 matrix of float.
    pub const MATRIX33: Self =
        Self::new(BaseType::Float, Aggregate::Matrix33, VecSemantics::NoXform);
    /// float×2 (no semantics).
    pub const FLOAT2: Self = Self::new(BaseType::Float, Aggregate::Vec2, VecSemantics::NoXform);
    /// float×4 (no semantics).
    pub const FLOAT4: Self = Self::new(BaseType::Float, Aggregate::Vec4, VecSemantics::NoXform);
    /// Vector2 (float×2, vector semantics).
    pub const VECTOR2: Self = Self::new(BaseType::Float, Aggregate::Vec2, VecSemantics::Vector);
    /// Vector4 (float×4, vector semantics).
    pub const VECTOR4: Self = Self::new(BaseType::Float, Aggregate::Vec4, VecSemantics::Vector);
    /// UInt64 scalar.
    pub const UINT64: Self = Self::scalar(BaseType::UInt64);

    /// Get the basetype enum.
    #[inline]
    pub const fn base_type(&self) -> BaseType {
        BaseType::from_u8(self.basetype)
    }

    /// Get the aggregate enum.
    #[inline]
    pub const fn agg(&self) -> Aggregate {
        Aggregate::from_u8(self.aggregate)
    }

    /// Get the vector semantics enum.
    #[inline]
    pub const fn vec_semantics(&self) -> VecSemantics {
        VecSemantics::from_u8(self.vecsemantics)
    }

    /// Is this an array type?
    #[inline]
    pub const fn is_array(&self) -> bool {
        self.arraylen != 0
    }

    /// Is this an unsized (variable-length) array?
    #[inline]
    pub const fn is_unsized_array(&self) -> bool {
        self.arraylen < 0
    }

    /// Is this a sized array?
    #[inline]
    pub const fn is_sized_array(&self) -> bool {
        self.arraylen > 0
    }

    /// Number of elements in the type. Returns max(1, arraylen) for arrays.
    #[inline]
    pub const fn numelements(&self) -> usize {
        if self.arraylen > 0 {
            self.arraylen as usize
        } else {
            1
        }
    }

    /// Total number of base-type values (elements × aggregate count).
    #[inline]
    pub const fn basevalues(&self) -> usize {
        self.numelements() * Aggregate::from_u8(self.aggregate).count()
    }

    /// Size in bytes of one element (without array).
    #[inline]
    pub const fn elementsize(&self) -> usize {
        BaseType::from_u8(self.basetype).size() * Aggregate::from_u8(self.aggregate).count()
    }

    /// Total size in bytes of this type (including array).
    #[inline]
    pub const fn size(&self) -> usize {
        self.elementsize() * self.numelements()
    }

    /// Return the element type (strip array).
    #[inline]
    pub const fn elementtype(&self) -> Self {
        Self {
            basetype: self.basetype,
            aggregate: self.aggregate,
            vecsemantics: self.vecsemantics,
            reserved: 0,
            arraylen: 0,
        }
    }

    /// Is this a Vec3-like type (point, vector, normal, or color)?
    #[inline]
    pub const fn is_vec3(&self) -> bool {
        self.aggregate == Aggregate::Vec3 as u8
    }

    /// Is this a float-based type?
    #[inline]
    pub const fn is_float_based(&self) -> bool {
        self.basetype == BaseType::Float as u8
    }

    /// Is this a single scalar float?
    #[inline]
    pub const fn is_float(&self) -> bool {
        self.basetype == BaseType::Float as u8
            && self.aggregate == Aggregate::Scalar as u8
            && self.arraylen == 0
    }

    /// Is this a single scalar int?
    #[inline]
    pub const fn is_int(&self) -> bool {
        self.basetype == BaseType::Int32 as u8
            && self.aggregate == Aggregate::Scalar as u8
            && self.arraylen == 0
    }

    /// Is this a single string?
    #[inline]
    pub const fn is_string(&self) -> bool {
        self.basetype == BaseType::String as u8
            && self.aggregate == Aggregate::Scalar as u8
            && self.arraylen == 0
    }

    /// Is this a triple (color, point, vector, or normal) — non-array?
    #[inline]
    pub const fn is_triple(&self) -> bool {
        self.basetype == BaseType::Float as u8
            && self.aggregate == Aggregate::Vec3 as u8
            && self.arraylen == 0
    }

    /// Is this a matrix (4×4)?
    #[inline]
    pub const fn is_matrix44(&self) -> bool {
        self.basetype == BaseType::Float as u8
            && self.aggregate == Aggregate::Matrix44 as u8
            && self.arraylen == 0
    }

    /// Serialize to the POD `i64` representation (little-endian field packing).
    ///
    /// # Why not `transmute`?
    ///
    /// `transmute` is `unsafe` and depends on host endianness. Explicit
    /// bit-packing is safe, portable, and the compiler optimizes it to
    /// the same single load/store on little-endian targets.
    #[inline]
    pub const fn to_pod(&self) -> TypeDescPod {
        (self.basetype as i64)
            | ((self.aggregate as i64) << 8)
            | ((self.vecsemantics as i64) << 16)
            | ((self.reserved as i64) << 24)
            | ((self.arraylen as i64) << 32)
    }

    /// Deserialize from the POD `i64` representation.
    #[inline]
    pub const fn from_pod(pod: TypeDescPod) -> Self {
        Self {
            basetype: pod as u8,
            aggregate: (pod >> 8) as u8,
            vecsemantics: (pod >> 16) as u8,
            reserved: (pod >> 24) as u8,
            arraylen: (pod >> 32) as i32,
        }
    }
}

impl Default for TypeDesc {
    fn default() -> Self {
        Self::UNKNOWN
    }
}

impl fmt::Debug for TypeDesc {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "TypeDesc({self})")
    }
}

impl fmt::Display for TypeDesc {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let bt = self.base_type();
        let ag = self.agg();
        let vs = self.vec_semantics();

        // Try well-known named types first
        if self.arraylen == 0 {
            match (bt, ag, vs) {
                (BaseType::Float, Aggregate::Scalar, _) => return write!(f, "float"),
                (BaseType::Int32, Aggregate::Scalar, _) => return write!(f, "int"),
                (BaseType::String, Aggregate::Scalar, _) => return write!(f, "string"),
                (BaseType::Float, Aggregate::Vec3, VecSemantics::Color) => {
                    return write!(f, "color");
                }
                (BaseType::Float, Aggregate::Vec3, VecSemantics::Point) => {
                    return write!(f, "point");
                }
                (BaseType::Float, Aggregate::Vec3, VecSemantics::Vector) => {
                    return write!(f, "vector");
                }
                (BaseType::Float, Aggregate::Vec3, VecSemantics::Normal) => {
                    return write!(f, "normal");
                }
                (BaseType::Float, Aggregate::Matrix44, _) => return write!(f, "matrix"),
                (BaseType::None, Aggregate::Scalar, _) => return write!(f, "void"),
                _ => {}
            }
        }

        // Generic fallback
        let base_name = match bt {
            BaseType::Unknown => "unknown",
            BaseType::None => "void",
            BaseType::UInt8 => "uint8",
            BaseType::Int8 => "int8",
            BaseType::UInt16 => "uint16",
            BaseType::Int16 => "int16",
            BaseType::UInt32 => "uint32",
            BaseType::Int32 => "int",
            BaseType::UInt64 => "uint64",
            BaseType::Int64 => "int64",
            BaseType::Half => "half",
            BaseType::Float => "float",
            BaseType::Double => "double",
            BaseType::String => "string",
            BaseType::Ptr => "ptr",
            BaseType::UStringHash => "ustringhash",
        };

        write!(f, "{base_name}")?;

        if ag != Aggregate::Scalar {
            write!(f, "[{:?}]", ag)?;
        }
        if vs != VecSemantics::NoXform {
            write!(f, "({:?})", vs)?;
        }
        if self.arraylen > 0 {
            write!(f, "[{}]", self.arraylen)?;
        } else if self.arraylen < 0 {
            write!(f, "[]")?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_size() {
        assert_eq!(std::mem::size_of::<TypeDesc>(), 8);
    }

    #[test]
    fn test_pod_roundtrip() {
        let types = [
            TypeDesc::FLOAT,
            TypeDesc::INT,
            TypeDesc::STRING,
            TypeDesc::COLOR,
            TypeDesc::POINT,
            TypeDesc::VECTOR,
            TypeDesc::NORMAL,
            TypeDesc::MATRIX,
            TypeDesc::FLOAT2,
            TypeDesc::FLOAT4,
        ];
        for &t in &types {
            let pod = t.to_pod();
            let t2 = TypeDesc::from_pod(pod);
            assert_eq!(t, t2, "pod roundtrip failed for {t}");
        }
    }

    #[test]
    fn test_sizes() {
        assert_eq!(TypeDesc::FLOAT.size(), 4);
        assert_eq!(TypeDesc::INT.size(), 4);
        assert_eq!(TypeDesc::COLOR.size(), 12);
        assert_eq!(TypeDesc::MATRIX.size(), 64);
        assert_eq!(TypeDesc::FLOAT.array(10).size(), 40);
        assert_eq!(TypeDesc::COLOR.array(3).size(), 36);
    }

    #[test]
    fn test_predicates() {
        assert!(TypeDesc::FLOAT.is_float());
        assert!(TypeDesc::INT.is_int());
        assert!(TypeDesc::STRING.is_string());
        assert!(TypeDesc::COLOR.is_triple());
        assert!(TypeDesc::POINT.is_triple());
        assert!(TypeDesc::VECTOR.is_triple());
        assert!(TypeDesc::NORMAL.is_triple());
        assert!(TypeDesc::MATRIX.is_matrix44());
        assert!(!TypeDesc::FLOAT.is_array());
        assert!(TypeDesc::FLOAT.array(5).is_array());
        assert!(TypeDesc::FLOAT.array(-1).is_unsized_array());
    }

    #[test]
    fn test_string_size() {
        // STRING is pointer-based (UString = thin pointer), must be 8 on 64-bit.
        assert_eq!(TypeDesc::STRING.size(), std::mem::size_of::<usize>());
        assert_eq!(TypeDesc::PTR.size(), std::mem::size_of::<usize>());
    }

    #[test]
    fn test_display() {
        assert_eq!(format!("{}", TypeDesc::FLOAT), "float");
        assert_eq!(format!("{}", TypeDesc::INT), "int");
        assert_eq!(format!("{}", TypeDesc::STRING), "string");
        assert_eq!(format!("{}", TypeDesc::COLOR), "color");
        assert_eq!(format!("{}", TypeDesc::POINT), "point");
        assert_eq!(format!("{}", TypeDesc::VECTOR), "vector");
        assert_eq!(format!("{}", TypeDesc::NORMAL), "normal");
        assert_eq!(format!("{}", TypeDesc::MATRIX), "matrix");
        assert_eq!(format!("{}", TypeDesc::NONE), "void");
    }
}
