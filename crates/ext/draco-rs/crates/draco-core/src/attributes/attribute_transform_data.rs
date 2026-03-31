//! Attribute transform data container.
//! Reference: `_ref/draco/src/draco/attributes/attribute_transform_data.h`.

use crate::attributes::attribute_transform_type::AttributeTransformType;
use crate::core::data_buffer::DataBuffer;

#[derive(Clone, Debug, Default)]
pub struct AttributeTransformData {
    transform_type: AttributeTransformType,
    buffer: DataBuffer,
}

impl AttributeTransformData {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn transform_type(&self) -> AttributeTransformType {
        self.transform_type
    }

    pub fn set_transform_type(&mut self, t: AttributeTransformType) {
        self.transform_type = t;
    }

    pub fn get_parameter_value<T: Copy>(&self, byte_offset: i32) -> T {
        let size = std::mem::size_of::<T>();
        let mut tmp = vec![0u8; size];
        self.buffer.read(byte_offset as i64, &mut tmp);
        let mut out = std::mem::MaybeUninit::<T>::uninit();
        unsafe {
            std::ptr::copy_nonoverlapping(tmp.as_ptr(), out.as_mut_ptr() as *mut u8, size);
            out.assume_init()
        }
    }

    pub fn set_parameter_value<T: Copy>(&mut self, byte_offset: i32, in_data: &T) {
        let size = std::mem::size_of::<T>();
        let required = (byte_offset as i64).saturating_add(size as i64);
        if required > self.buffer.data_size() as i64 {
            self.buffer.resize(required);
        }
        let bytes = unsafe { std::slice::from_raw_parts(in_data as *const T as *const u8, size) };
        self.buffer.write(byte_offset as i64, &bytes);
    }

    pub fn append_parameter_value<T: Copy>(&mut self, in_data: &T) {
        let offset = self.buffer.data_size() as i32;
        self.set_parameter_value(offset, in_data);
    }
}
