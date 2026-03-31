//! JavaScript-facing constants and enum conversions for Draco WASM bindings.
//!
//! These constants mirror the draco3d Emscripten bindings so JavaScript can
//! reference familiar names like `POSITION` or `DT_FLOAT32`.

use draco_bitstream::compression::config::compression_shared::{
    EncodedGeometryType as CoreEncodedGeometryType, MeshEncoderMethod as CoreMeshEncoderMethod,
};
use draco_core::attributes::attribute_transform_type::AttributeTransformType as CoreTransformType;
use draco_core::attributes::geometry_attribute::GeometryAttributeType as CoreAttributeType;
use draco_core::core::draco_types::DataType as CoreDataType;
use draco_core::core::status::StatusCode as CoreStatusCode;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
pub const ATTRIBUTE_INVALID_TRANSFORM: i32 = CoreTransformType::InvalidTransform as i32;
#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
pub const ATTRIBUTE_NO_TRANSFORM: i32 = CoreTransformType::NoTransform as i32;
#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
pub const ATTRIBUTE_QUANTIZATION_TRANSFORM: i32 = CoreTransformType::QuantizationTransform as i32;
#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
pub const ATTRIBUTE_OCTAHEDRON_TRANSFORM: i32 = CoreTransformType::OctahedronTransform as i32;

#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
pub const INVALID: i32 = CoreAttributeType::Invalid as i32;
#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
pub const POSITION: i32 = CoreAttributeType::Position as i32;
#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
pub const NORMAL: i32 = CoreAttributeType::Normal as i32;
#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
pub const COLOR: i32 = CoreAttributeType::Color as i32;
#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
pub const TEX_COORD: i32 = CoreAttributeType::TexCoord as i32;
#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
pub const GENERIC: i32 = CoreAttributeType::Generic as i32;

#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
pub const INVALID_GEOMETRY_TYPE: i32 = CoreEncodedGeometryType::InvalidGeometryType as i32;
#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
pub const POINT_CLOUD: i32 = CoreEncodedGeometryType::PointCloud as i32;
#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
pub const TRIANGULAR_MESH: i32 = CoreEncodedGeometryType::TriangularMesh as i32;

#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
pub const MESH_SEQUENTIAL_ENCODING: i32 = CoreMeshEncoderMethod::MeshSequentialEncoding as i32;
#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
pub const MESH_EDGEBREAKER_ENCODING: i32 = CoreMeshEncoderMethod::MeshEdgebreakerEncoding as i32;

#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
pub const DT_INVALID: i32 = 0;
#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
pub const DT_INT8: i32 = 1;
#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
pub const DT_UINT8: i32 = 2;
#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
pub const DT_INT16: i32 = 3;
#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
pub const DT_UINT16: i32 = 4;
#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
pub const DT_INT32: i32 = 5;
#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
pub const DT_UINT32: i32 = 6;
#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
pub const DT_INT64: i32 = 7;
#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
pub const DT_UINT64: i32 = 8;
#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
pub const DT_FLOAT32: i32 = 9;
#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
pub const DT_FLOAT64: i32 = 10;
#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
pub const DT_BOOL: i32 = 11;
#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
pub const DT_TYPES_COUNT: i32 = 12;

#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
pub const OK: i32 = 0;
#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
pub const DRACO_ERROR: i32 = -1;
#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
pub const IO_ERROR: i32 = -2;
#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
pub const INVALID_PARAMETER: i32 = -3;
#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
pub const UNSUPPORTED_VERSION: i32 = -4;
#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
pub const UNKNOWN_VERSION: i32 = -5;

pub fn geometry_attribute_from_i32(value: i32) -> CoreAttributeType {
    match value {
        0 => CoreAttributeType::Position,
        1 => CoreAttributeType::Normal,
        2 => CoreAttributeType::Color,
        3 => CoreAttributeType::TexCoord,
        4 => CoreAttributeType::Generic,
        _ => CoreAttributeType::Invalid,
    }
}

pub fn data_type_from_i32(value: i32) -> CoreDataType {
    match value {
        1 => CoreDataType::Int8,
        2 => CoreDataType::Uint8,
        3 => CoreDataType::Int16,
        4 => CoreDataType::Uint16,
        5 => CoreDataType::Int32,
        6 => CoreDataType::Uint32,
        7 => CoreDataType::Int64,
        8 => CoreDataType::Uint64,
        9 => CoreDataType::Float32,
        10 => CoreDataType::Float64,
        11 => CoreDataType::Bool,
        12 => CoreDataType::TypesCount,
        _ => CoreDataType::Invalid,
    }
}

pub fn data_type_to_i32(value: CoreDataType) -> i32 {
    match value {
        CoreDataType::Invalid => 0,
        CoreDataType::Int8 => 1,
        CoreDataType::Uint8 => 2,
        CoreDataType::Int16 => 3,
        CoreDataType::Uint16 => 4,
        CoreDataType::Int32 => 5,
        CoreDataType::Uint32 => 6,
        CoreDataType::Int64 => 7,
        CoreDataType::Uint64 => 8,
        CoreDataType::Float32 => 9,
        CoreDataType::Float64 => 10,
        CoreDataType::Bool => 11,
        CoreDataType::TypesCount => 12,
    }
}

pub fn transform_type_to_i32(value: CoreTransformType) -> i32 {
    value as i32
}

pub fn encoded_geometry_to_i32(value: CoreEncodedGeometryType) -> i32 {
    value as i32
}

pub fn mesh_encoder_method_from_i32(value: i32) -> CoreMeshEncoderMethod {
    match value {
        1 => CoreMeshEncoderMethod::MeshEdgebreakerEncoding,
        _ => CoreMeshEncoderMethod::MeshSequentialEncoding,
    }
}

pub fn status_code_to_i32(value: CoreStatusCode) -> i32 {
    match value {
        CoreStatusCode::Ok => 0,
        CoreStatusCode::DracoError => -1,
        CoreStatusCode::IoError => -2,
        CoreStatusCode::InvalidParameter => -3,
        CoreStatusCode::UnsupportedVersion => -4,
        CoreStatusCode::UnknownVersion => -5,
        CoreStatusCode::UnsupportedFeature => -6,
    }
}
