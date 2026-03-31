//! PLY property reader.
//! Reference: `_ref/draco/src/draco/io/ply_property_reader.h`.

use crate::core::draco_types::DataType;
use crate::io::ply_reader::PlyProperty;

pub trait PlyReadCast: Copy + Default {
    fn from_i64(value: i64) -> Self;
    fn from_f64(value: f64) -> Self;
}

impl PlyReadCast for u8 {
    fn from_i64(value: i64) -> Self {
        value as u8
    }
    fn from_f64(value: f64) -> Self {
        value as u8
    }
}
impl PlyReadCast for u16 {
    fn from_i64(value: i64) -> Self {
        value as u16
    }
    fn from_f64(value: f64) -> Self {
        value as u16
    }
}
impl PlyReadCast for u32 {
    fn from_i64(value: i64) -> Self {
        value as u32
    }
    fn from_f64(value: f64) -> Self {
        value as u32
    }
}
impl PlyReadCast for i8 {
    fn from_i64(value: i64) -> Self {
        value as i8
    }
    fn from_f64(value: f64) -> Self {
        value as i8
    }
}
impl PlyReadCast for i16 {
    fn from_i64(value: i64) -> Self {
        value as i16
    }
    fn from_f64(value: f64) -> Self {
        value as i16
    }
}
impl PlyReadCast for i32 {
    fn from_i64(value: i64) -> Self {
        value as i32
    }
    fn from_f64(value: f64) -> Self {
        value as i32
    }
}
impl PlyReadCast for f32 {
    fn from_i64(value: i64) -> Self {
        value as f32
    }
    fn from_f64(value: f64) -> Self {
        value as f32
    }
}
impl PlyReadCast for f64 {
    fn from_i64(value: i64) -> Self {
        value as f64
    }
    fn from_f64(value: f64) -> Self {
        value
    }
}

pub struct PlyPropertyReader<'a, T: PlyReadCast> {
    property: &'a PlyProperty,
    _phantom: std::marker::PhantomData<T>,
}

impl<'a, T: PlyReadCast> PlyPropertyReader<'a, T> {
    pub fn new(property: &'a PlyProperty) -> Self {
        Self {
            property,
            _phantom: std::marker::PhantomData,
        }
    }

    pub fn read_value(&self, value_id: i32) -> T {
        let num_bytes = self.property.data_type_num_bytes() as usize;
        if num_bytes == 0 {
            return T::default();
        }
        let offset = (value_id as usize).saturating_mul(num_bytes);
        if offset + num_bytes > self.property.data().len() {
            return T::default();
        }
        let bytes = &self.property.data()[offset..offset + num_bytes];
        match self.property.data_type() {
            DataType::Uint8 => T::from_i64(bytes[0] as i64),
            DataType::Int8 => T::from_i64((bytes[0] as i8) as i64),
            DataType::Uint16 => {
                let v = u16::from_le_bytes([bytes[0], bytes[1]]);
                T::from_i64(v as i64)
            }
            DataType::Int16 => {
                let v = i16::from_le_bytes([bytes[0], bytes[1]]);
                T::from_i64(v as i64)
            }
            DataType::Uint32 => {
                let v = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
                T::from_i64(v as i64)
            }
            DataType::Int32 => {
                let v = i32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
                T::from_i64(v as i64)
            }
            DataType::Float32 => {
                let v = f32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
                T::from_f64(v as f64)
            }
            DataType::Float64 => {
                let v = f64::from_le_bytes([
                    bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
                ]);
                T::from_f64(v)
            }
            _ => T::default(),
        }
    }
}
