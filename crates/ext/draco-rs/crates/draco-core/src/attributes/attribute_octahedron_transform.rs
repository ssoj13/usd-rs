//! Attribute octahedron transform.
//! Reference: `_ref/draco/src/draco/attributes/attribute_octahedron_transform.h` + `.cc`.

use crate::attributes::attribute_transform::AttributeTransform;
use crate::attributes::attribute_transform_data::AttributeTransformData;
use crate::attributes::attribute_transform_type::AttributeTransformType;
use crate::attributes::geometry_indices::{AttributeValueIndex, PointIndex};
use crate::attributes::point_attribute::PointAttribute;
use crate::compression::attributes::normal_compression_utils::OctahedronToolBox;
use crate::core::decoder_buffer::DecoderBuffer;
use crate::core::draco_types::DataType;
use crate::core::encoder_buffer::EncoderBuffer;
use crate::draco_dcheck;

#[derive(Clone, Debug)]
pub struct AttributeOctahedronTransform {
    quantization_bits: i32,
}

impl AttributeOctahedronTransform {
    pub fn new() -> Self {
        Self {
            quantization_bits: -1,
        }
    }

    pub fn set_parameters(&mut self, quantization_bits: i32) {
        self.quantization_bits = quantization_bits;
    }

    pub fn is_initialized(&self) -> bool {
        self.quantization_bits != -1
    }

    pub fn quantization_bits(&self) -> i32 {
        self.quantization_bits
    }

    fn generate_portable_attribute(
        &self,
        attribute: &PointAttribute,
        point_ids: &[PointIndex],
        num_points: usize,
        target_attribute: &mut PointAttribute,
    ) -> bool {
        draco_dcheck!(self.is_initialized());
        let mut converter = OctahedronToolBox::new();
        if !converter.set_quantization_bits(self.quantization_bits) {
            return false;
        }
        if point_ids.is_empty() {
            for i in 0..num_points {
                let att_val_id = attribute.mapped_index(PointIndex::from(i as u32));
                let att_val = attribute.get_value_array::<f32, 3>(att_val_id);
                let mut s = 0i32;
                let mut t = 0i32;
                converter.float_vector_to_quantized_octahedral_coords(&att_val, &mut s, &mut t);
                let out = [s, t];
                target_attribute
                    .set_attribute_value_array::<i32, 2>(AttributeValueIndex::from(i as u32), &out);
            }
        } else {
            for (dst_index, point_id) in point_ids.iter().enumerate() {
                let att_val_id = attribute.mapped_index(*point_id);
                let att_val = attribute.get_value_array::<f32, 3>(att_val_id);
                let mut s = 0i32;
                let mut t = 0i32;
                converter.float_vector_to_quantized_octahedral_coords(&att_val, &mut s, &mut t);
                let out = [s, t];
                target_attribute.set_attribute_value_array::<i32, 2>(
                    AttributeValueIndex::from(dst_index as u32),
                    &out,
                );
            }
        }
        true
    }
}

impl Default for AttributeOctahedronTransform {
    fn default() -> Self {
        Self::new()
    }
}

impl AttributeTransform for AttributeOctahedronTransform {
    fn transform_type(&self) -> AttributeTransformType {
        AttributeTransformType::OctahedronTransform
    }

    fn init_from_attribute(&mut self, attribute: &PointAttribute) -> bool {
        let transform_data = match attribute.get_attribute_transform_data() {
            Some(data) => data,
            None => return false,
        };
        if transform_data.transform_type() != AttributeTransformType::OctahedronTransform {
            return false;
        }
        self.quantization_bits = transform_data.get_parameter_value::<i32>(0);
        true
    }

    fn copy_to_attribute_transform_data(&self, out_data: &mut AttributeTransformData) {
        out_data.set_transform_type(AttributeTransformType::OctahedronTransform);
        out_data.append_parameter_value(&self.quantization_bits);
    }

    fn transform_attribute(
        &self,
        attribute: &PointAttribute,
        point_ids: &[PointIndex],
        target_attribute: &mut PointAttribute,
    ) -> bool {
        self.generate_portable_attribute(
            attribute,
            point_ids,
            target_attribute.size(),
            target_attribute,
        )
    }

    fn inverse_transform_attribute(
        &self,
        attribute: &PointAttribute,
        target_attribute: &mut PointAttribute,
    ) -> bool {
        if target_attribute.data_type() != DataType::Float32 {
            return false;
        }
        let num_points = target_attribute.size();
        let num_components = target_attribute.num_components();
        if num_components != 3 {
            return false;
        }

        let entry_size = std::mem::size_of::<f32>() * 3;
        let mut att_val = [0.0f32; 3];
        let mut octahedron_tool_box = OctahedronToolBox::new();
        if !octahedron_tool_box.set_quantization_bits(self.quantization_bits) {
            return false;
        }

        let buffer = match attribute.buffer() {
            Some(buf) => buf,
            None => return false,
        };
        let buf_ref = buffer.borrow();
        let data = buf_ref.data();
        let required = num_points * 2 * std::mem::size_of::<i32>();
        if data.len() < required {
            return false;
        }

        let mut offset = 0usize;
        for i in 0..num_points {
            let mut s_bytes = [0u8; 4];
            let mut t_bytes = [0u8; 4];
            s_bytes.copy_from_slice(&data[offset..offset + 4]);
            offset += 4;
            t_bytes.copy_from_slice(&data[offset..offset + 4]);
            offset += 4;
            let s = i32::from_ne_bytes(s_bytes);
            let t = i32::from_ne_bytes(t_bytes);

            octahedron_tool_box.quantized_octahedral_coords_to_unit_vector(s, t, &mut att_val);
            let bytes =
                unsafe { std::slice::from_raw_parts(att_val.as_ptr() as *const u8, entry_size) };
            target_attribute.set_attribute_value_bytes(AttributeValueIndex::from(i as u32), bytes);
        }
        true
    }

    fn encode_parameters(&self, encoder_buffer: &mut EncoderBuffer) -> bool {
        if self.is_initialized() {
            encoder_buffer.encode(self.quantization_bits as u8);
            return true;
        }
        false
    }

    fn decode_parameters(
        &mut self,
        _attribute: &PointAttribute,
        decoder_buffer: &mut DecoderBuffer,
    ) -> bool {
        let mut quantization_bits: u8 = 0;
        if !decoder_buffer.decode(&mut quantization_bits) {
            return false;
        }
        self.quantization_bits = quantization_bits as i32;
        true
    }

    fn get_transformed_data_type(&self, _attribute: &PointAttribute) -> DataType {
        DataType::Uint32
    }

    fn get_transformed_num_components(&self, _attribute: &PointAttribute) -> i32 {
        2
    }
}
