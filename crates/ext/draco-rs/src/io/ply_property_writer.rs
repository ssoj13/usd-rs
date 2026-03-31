//! PLY property writer.
//! Reference: `_ref/draco/src/draco/io/ply_property_writer.h`.

use crate::core::draco_types::DataType;
use crate::io::ply_reader::PlyProperty;

pub trait PlyWriteCast: Copy {
    fn to_i64(self) -> i64;
    fn to_f64(self) -> f64;
}

impl PlyWriteCast for f64 {
    fn to_i64(self) -> i64 {
        self as i64
    }
    fn to_f64(self) -> f64 {
        self
    }
}
impl PlyWriteCast for f32 {
    fn to_i64(self) -> i64 {
        self as i64
    }
    fn to_f64(self) -> f64 {
        self as f64
    }
}
impl PlyWriteCast for i64 {
    fn to_i64(self) -> i64 {
        self
    }
    fn to_f64(self) -> f64 {
        self as f64
    }
}
impl PlyWriteCast for i32 {
    fn to_i64(self) -> i64 {
        self as i64
    }
    fn to_f64(self) -> f64 {
        self as f64
    }
}
impl PlyWriteCast for u32 {
    fn to_i64(self) -> i64 {
        self as i64
    }
    fn to_f64(self) -> f64 {
        self as f64
    }
}
impl PlyWriteCast for u16 {
    fn to_i64(self) -> i64 {
        self as i64
    }
    fn to_f64(self) -> f64 {
        self as f64
    }
}
impl PlyWriteCast for u8 {
    fn to_i64(self) -> i64 {
        self as i64
    }
    fn to_f64(self) -> f64 {
        self as f64
    }
}

pub struct PlyPropertyWriter<'a, T: PlyWriteCast> {
    property: &'a mut PlyProperty,
    _phantom: std::marker::PhantomData<T>,
}

impl<'a, T: PlyWriteCast> PlyPropertyWriter<'a, T> {
    pub fn new(property: &'a mut PlyProperty) -> Self {
        Self {
            property,
            _phantom: std::marker::PhantomData,
        }
    }

    pub fn push_back_value(&mut self, value: T) {
        match self.property.data_type() {
            DataType::Uint8 => {
                let v = value.to_i64() as u8;
                self.property.push_back_value_bytes(&v.to_le_bytes());
            }
            DataType::Int8 => {
                let v = value.to_i64() as i8;
                self.property.push_back_value_bytes(&v.to_le_bytes());
            }
            DataType::Uint16 => {
                let v = value.to_i64() as u16;
                self.property.push_back_value_bytes(&v.to_le_bytes());
            }
            DataType::Int16 => {
                let v = value.to_i64() as i16;
                self.property.push_back_value_bytes(&v.to_le_bytes());
            }
            DataType::Uint32 => {
                let v = value.to_i64() as u32;
                self.property.push_back_value_bytes(&v.to_le_bytes());
            }
            DataType::Int32 => {
                let v = value.to_i64() as i32;
                self.property.push_back_value_bytes(&v.to_le_bytes());
            }
            DataType::Float32 => {
                let v = value.to_f64() as f32;
                self.property.push_back_value_bytes(&v.to_le_bytes());
            }
            DataType::Float64 => {
                let v = value.to_f64();
                self.property.push_back_value_bytes(&v.to_le_bytes());
            }
            _ => {}
        }
    }
}
