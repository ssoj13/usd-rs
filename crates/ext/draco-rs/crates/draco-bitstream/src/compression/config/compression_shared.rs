//! Shared compression constants and enums.
//! Reference: `_ref/draco/src/draco/compression/config/compression_shared.h`.

/// Converts Draco major/minor into a bitstream version.
#[inline]
pub const fn bitstream_version(major: u8, minor: u8) -> u16 {
    ((major as u16) << 8) | minor as u16
}

// Latest Draco bit-stream version.
pub const K_DRACO_POINT_CLOUD_BITSTREAM_VERSION_MAJOR: u8 = 2;
pub const K_DRACO_POINT_CLOUD_BITSTREAM_VERSION_MINOR: u8 = 3;
pub const K_DRACO_MESH_BITSTREAM_VERSION_MAJOR: u8 = 2;
pub const K_DRACO_MESH_BITSTREAM_VERSION_MINOR: u8 = 2;

pub const K_DRACO_POINT_CLOUD_BITSTREAM_VERSION: u16 = bitstream_version(
    K_DRACO_POINT_CLOUD_BITSTREAM_VERSION_MAJOR,
    K_DRACO_POINT_CLOUD_BITSTREAM_VERSION_MINOR,
);

pub const K_DRACO_MESH_BITSTREAM_VERSION: u16 = bitstream_version(
    K_DRACO_MESH_BITSTREAM_VERSION_MAJOR,
    K_DRACO_MESH_BITSTREAM_VERSION_MINOR,
);

/// Currently supported geometry types for encoding.
#[repr(i32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EncodedGeometryType {
    InvalidGeometryType = -1,
    PointCloud = 0,
    TriangularMesh = 1,
    NumEncodedGeometryTypes = 2,
}

/// Encoding methods for point clouds.
#[repr(i8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PointCloudEncodingMethod {
    PointCloudSequentialEncoding = 0,
    PointCloudKdTreeEncoding = 1,
}

/// Encoding methods for meshes.
#[repr(i8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MeshEncoderMethod {
    MeshSequentialEncoding = 0,
    MeshEdgebreakerEncoding = 1,
}

/// Attribute encoder identifiers.
#[repr(i8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AttributeEncoderType {
    BasicAttributeEncoder = 0,
    MeshTraversalAttributeEncoder = 1,
    KdTreeAttributeEncoder = 2,
}

/// Sequential attribute encoder identifiers.
#[repr(i8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SequentialAttributeEncoderType {
    SequentialAttributeEncoderGeneric = 0,
    SequentialAttributeEncoderInteger = 1,
    SequentialAttributeEncoderQuantization = 2,
    SequentialAttributeEncoderNormals = 3,
}

/// Prediction schemes supported by Draco.
#[repr(i32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PredictionSchemeMethod {
    PredictionNone = -2,
    PredictionUndefined = -1,
    PredictionDifference = 0,
    MeshPredictionParallelogram = 1,
    MeshPredictionMultiParallelogram = 2,
    MeshPredictionTexCoordsDeprecated = 3,
    MeshPredictionConstrainedMultiParallelogram = 4,
    MeshPredictionTexCoordsPortable = 5,
    MeshPredictionGeometricNormal = 6,
    NumPredictionSchemes = 7,
}

/// Prediction scheme transform types.
#[repr(i32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PredictionSchemeTransformType {
    PredictionTransformNone = -1,
    PredictionTransformDelta = 0,
    PredictionTransformWrap = 1,
    PredictionTransformNormalOctahedron = 2,
    PredictionTransformNormalOctahedronCanonicalized = 3,
    NumPredictionSchemeTransformTypes = 4,
}

/// Mesh traversal methods.
#[repr(i8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MeshTraversalMethod {
    MeshTraversalDepthFirst = 0,
    MeshTraversalPredictionDegree = 1,
    NumTraversalMethods = 2,
}

/// Edgebreaker connectivity encoding methods.
#[repr(i8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MeshEdgebreakerConnectivityEncodingMethod {
    MeshEdgebreakerStandardEncoding = 0,
    MeshEdgebreakerPredictiveEncoding = 1,
    MeshEdgebreakerValenceEncoding = 2,
}

/// Draco header V1.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct DracoHeader {
    pub draco_string: [i8; 5],
    pub version_major: u8,
    pub version_minor: u8,
    pub encoder_type: u8,
    pub encoder_method: u8,
    pub flags: u16,
}

/// Normal prediction mode identifiers.
#[repr(i8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NormalPredictionMode {
    OneTriangle = 0,
    TriangleArea = 1,
}

/// Entropy coding methods.
#[repr(i8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SymbolCodingMethod {
    SymbolCodingTagged = 0,
    SymbolCodingRaw = 1,
    NumSymbolCodingMethods = 2,
}

/// Mask for setting and getting the bit for metadata in header flags.
pub const METADATA_FLAG_MASK: u16 = 0x8000;
