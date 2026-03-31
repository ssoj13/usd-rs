//! Draco data types.
//! Reference: `_ref/draco/src/draco/core/draco_types.h` + `.cc`.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DataType {
    Invalid = 0,
    Int8,
    Uint8,
    Int16,
    Uint16,
    Int32,
    Uint32,
    Int64,
    Uint64,
    Float32,
    Float64,
    Bool,
    TypesCount,
}

pub fn data_type_length(dt: DataType) -> i32 {
    match dt {
        DataType::Int8 | DataType::Uint8 => 1,
        DataType::Int16 | DataType::Uint16 => 2,
        DataType::Int32 | DataType::Uint32 => 4,
        DataType::Int64 | DataType::Uint64 => 8,
        DataType::Float32 => 4,
        DataType::Float64 => 8,
        DataType::Bool => 1,
        _ => -1,
    }
}

pub fn is_data_type_integral(dt: DataType) -> bool {
    matches!(
        dt,
        DataType::Int8
            | DataType::Uint8
            | DataType::Int16
            | DataType::Uint16
            | DataType::Int32
            | DataType::Uint32
            | DataType::Int64
            | DataType::Uint64
            | DataType::Bool
    )
}
