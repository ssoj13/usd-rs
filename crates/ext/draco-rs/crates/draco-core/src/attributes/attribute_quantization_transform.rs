//! Attribute quantization transform.
//! Reference: `_ref/draco/src/draco/attributes/attribute_quantization_transform.h` + `.cc`.

use crate::attributes::attribute_transform::AttributeTransform;
use crate::attributes::attribute_transform_data::AttributeTransformData;
use crate::attributes::attribute_transform_type::AttributeTransformType;
use crate::attributes::geometry_indices::{AttributeValueIndex, PointIndex};
use crate::attributes::point_attribute::PointAttribute;
use crate::core::decoder_buffer::DecoderBuffer;
use crate::core::draco_types::DataType;
use crate::core::encoder_buffer::EncoderBuffer;
use crate::core::quantization_utils::{Dequantizer, Quantizer};
use crate::draco_dcheck;

#[derive(Clone, Debug)]
pub struct AttributeQuantizationTransform {
    quantization_bits: i32,
    min_values: Vec<f32>,
    range: f32,
}

impl AttributeQuantizationTransform {
    pub fn new() -> Self {
        Self {
            quantization_bits: -1,
            min_values: Vec::new(),
            range: 0.0,
        }
    }

    pub fn quantization_bits(&self) -> i32 {
        self.quantization_bits
    }

    pub fn min_value(&self, axis: usize) -> f32 {
        self.min_values[axis]
    }

    pub fn min_values(&self) -> &[f32] {
        &self.min_values
    }

    pub fn range(&self) -> f32 {
        self.range
    }

    pub fn is_initialized(&self) -> bool {
        self.quantization_bits != -1
    }

    pub fn set_parameters(
        &mut self,
        quantization_bits: i32,
        min_values: &[f32],
        num_components: usize,
        range: f32,
    ) -> bool {
        if !Self::is_quantization_valid(quantization_bits) {
            return false;
        }
        self.quantization_bits = quantization_bits;
        self.min_values.clear();
        self.min_values
            .extend_from_slice(&min_values[..num_components]);
        self.range = range;
        true
    }

    pub fn compute_parameters(
        &mut self,
        attribute: &PointAttribute,
        quantization_bits: i32,
    ) -> bool {
        if self.quantization_bits != -1 {
            return false;
        }
        if !Self::is_quantization_valid(quantization_bits) {
            return false;
        }
        self.quantization_bits = quantization_bits;

        let num_components = attribute.num_components() as usize;
        self.range = 0.0;
        self.min_values = vec![0.0; num_components];
        let mut max_values = vec![0.0f32; num_components];
        let mut att_val = vec![0.0f32; num_components];

        Self::get_attribute_values(attribute, AttributeValueIndex::from(0u32), &mut att_val);
        self.min_values.copy_from_slice(&att_val);
        max_values.copy_from_slice(&att_val);

        for i in 1..attribute.size() {
            let avi = AttributeValueIndex::from(i as u32);
            Self::get_attribute_values(attribute, avi, &mut att_val);
            for c in 0..num_components {
                if att_val[c].is_nan() {
                    return false;
                }
                if self.min_values[c] > att_val[c] {
                    self.min_values[c] = att_val[c];
                }
                if max_values[c] < att_val[c] {
                    max_values[c] = att_val[c];
                }
            }
        }

        for c in 0..num_components {
            if self.min_values[c].is_nan()
                || self.min_values[c].is_infinite()
                || max_values[c].is_nan()
                || max_values[c].is_infinite()
            {
                return false;
            }
            let dif = max_values[c] - self.min_values[c];
            if dif > self.range {
                self.range = dif;
            }
        }

        if self.range == 0.0 {
            self.range = 1.0;
        }
        true
    }

    fn is_quantization_valid(quantization_bits: i32) -> bool {
        quantization_bits >= 1 && quantization_bits <= 30
    }

    fn get_attribute_values(attribute: &PointAttribute, avi: AttributeValueIndex, out: &mut [f32]) {
        let total = out.len() * std::mem::size_of::<f32>();
        let mut tmp = vec![0u8; total];
        attribute.get_value_bytes(avi, &mut tmp);
        unsafe {
            std::ptr::copy_nonoverlapping(tmp.as_ptr(), out.as_mut_ptr() as *mut u8, total);
        }
    }

    fn generate_portable_attribute(
        &self,
        attribute: &PointAttribute,
        point_ids: &[PointIndex],
        num_points: usize,
        target_attribute: &mut PointAttribute,
    ) {
        draco_dcheck!(self.is_initialized());
        let num_components = attribute.num_components() as usize;
        let max_quantized_value = (1u32 << (self.quantization_bits as u32)) - 1;
        let mut quantizer = Quantizer::new();
        quantizer.init_range(self.range(), max_quantized_value as i32);

        let mut att_val = vec![0.0f32; num_components];
        let mut out_vals = vec![0i32; num_components];

        if point_ids.is_empty() {
            for i in 0..num_points {
                let att_val_id = attribute.mapped_index(PointIndex::from(i as u32));
                Self::get_attribute_values(attribute, att_val_id, &mut att_val);
                for c in 0..num_components {
                    let value = att_val[c] - self.min_values[c];
                    out_vals[c] = quantizer.quantize_float(value);
                }
                let bytes = unsafe {
                    std::slice::from_raw_parts(out_vals.as_ptr() as *const u8, num_components * 4)
                };
                target_attribute
                    .set_attribute_value_bytes(AttributeValueIndex::from(i as u32), bytes);
            }
        } else {
            for (dst_index, &point_id) in point_ids.iter().enumerate() {
                let att_val_id = attribute.mapped_index(point_id);
                Self::get_attribute_values(attribute, att_val_id, &mut att_val);
                for c in 0..num_components {
                    let value = att_val[c] - self.min_values[c];
                    out_vals[c] = quantizer.quantize_float(value);
                }
                let bytes = unsafe {
                    std::slice::from_raw_parts(out_vals.as_ptr() as *const u8, num_components * 4)
                };
                target_attribute
                    .set_attribute_value_bytes(AttributeValueIndex::from(dst_index as u32), bytes);
            }
        }
    }
}

impl Default for AttributeQuantizationTransform {
    fn default() -> Self {
        Self::new()
    }
}

impl AttributeTransform for AttributeQuantizationTransform {
    fn transform_type(&self) -> AttributeTransformType {
        AttributeTransformType::QuantizationTransform
    }

    fn init_from_attribute(&mut self, attribute: &PointAttribute) -> bool {
        let transform_data = match attribute.get_attribute_transform_data() {
            Some(data) => data,
            None => return false,
        };
        if transform_data.transform_type() != AttributeTransformType::QuantizationTransform {
            return false;
        }
        let mut byte_offset = 0;
        self.quantization_bits = transform_data.get_parameter_value::<i32>(byte_offset);
        byte_offset += 4;
        self.min_values
            .resize(attribute.num_components() as usize, 0.0);
        for i in 0..attribute.num_components() as usize {
            self.min_values[i] = transform_data.get_parameter_value::<f32>(byte_offset);
            byte_offset += 4;
        }
        self.range = transform_data.get_parameter_value::<f32>(byte_offset);
        true
    }

    fn copy_to_attribute_transform_data(&self, out_data: &mut AttributeTransformData) {
        out_data.set_transform_type(AttributeTransformType::QuantizationTransform);
        out_data.append_parameter_value(&self.quantization_bits);
        for v in &self.min_values {
            out_data.append_parameter_value(v);
        }
        out_data.append_parameter_value(&self.range);
    }

    fn transform_attribute(
        &self,
        attribute: &PointAttribute,
        point_ids: &[PointIndex],
        target_attribute: &mut PointAttribute,
    ) -> bool {
        if point_ids.is_empty() {
            self.generate_portable_attribute(
                attribute,
                point_ids,
                target_attribute.size(),
                target_attribute,
            );
        } else {
            self.generate_portable_attribute(
                attribute,
                point_ids,
                target_attribute.size(),
                target_attribute,
            );
        }
        true
    }

    fn inverse_transform_attribute(
        &self,
        attribute: &PointAttribute,
        target_attribute: &mut PointAttribute,
    ) -> bool {
        if target_attribute.data_type() != DataType::Float32 {
            return false;
        }
        let max_quantized_value = (1u32 << (self.quantization_bits as u32)) - 1;
        let num_components = target_attribute.num_components() as usize;
        let entry_size = std::mem::size_of::<f32>() * num_components;
        let mut att_val = vec![0.0f32; num_components];
        let mut dequantizer = Dequantizer::new();
        if !dequantizer.init_range(self.range, max_quantized_value as i32) {
            return false;
        }

        let num_values = target_attribute.size();
        let mut quant_val_id = 0usize;

        for i in 0..num_values {
            for c in 0..num_components {
                let q = Self::get_quantized_value(attribute, quant_val_id);
                quant_val_id += 1;
                let mut value = dequantizer.dequantize_float(q);
                value += self.min_values[c];
                att_val[c] = value;
            }
            let bytes =
                unsafe { std::slice::from_raw_parts(att_val.as_ptr() as *const u8, entry_size) };
            target_attribute.set_attribute_value_bytes(AttributeValueIndex::from(i as u32), bytes);
        }
        true
    }

    fn encode_parameters(&self, encoder_buffer: &mut EncoderBuffer) -> bool {
        if self.is_initialized() {
            let bytes = unsafe {
                std::slice::from_raw_parts(
                    self.min_values.as_ptr() as *const u8,
                    self.min_values.len() * std::mem::size_of::<f32>(),
                )
            };
            encoder_buffer.encode_bytes(bytes);
            encoder_buffer.encode(self.range);
            encoder_buffer.encode(self.quantization_bits as u8);
            return true;
        }
        false
    }

    fn decode_parameters(
        &mut self,
        attribute: &PointAttribute,
        decoder_buffer: &mut DecoderBuffer,
    ) -> bool {
        self.min_values
            .resize(attribute.num_components() as usize, 0.0);
        let bytes = unsafe {
            std::slice::from_raw_parts_mut(
                self.min_values.as_mut_ptr() as *mut u8,
                self.min_values.len() * std::mem::size_of::<f32>(),
            )
        };
        if !decoder_buffer.decode_bytes(bytes) {
            return false;
        }
        if !decoder_buffer.decode(&mut self.range) {
            return false;
        }
        let mut quantization_bits: u8 = 0;
        if !decoder_buffer.decode(&mut quantization_bits) {
            return false;
        }
        if !Self::is_quantization_valid(quantization_bits as i32) {
            return false;
        }
        self.quantization_bits = quantization_bits as i32;
        true
    }

    fn get_transformed_data_type(&self, _attribute: &PointAttribute) -> DataType {
        DataType::Uint32
    }

    fn get_transformed_num_components(&self, attribute: &PointAttribute) -> i32 {
        attribute.num_components() as i32
    }
}

impl AttributeQuantizationTransform {
    fn get_quantized_value(attribute: &PointAttribute, index: usize) -> i32 {
        let num_components = attribute.num_components() as usize;
        let entry_size = std::mem::size_of::<i32>() * num_components;
        let value_index = index / num_components;
        let component = index % num_components;
        let mut tmp = vec![0u8; entry_size];
        attribute.get_value_bytes(AttributeValueIndex::from(value_index as u32), &mut tmp);
        let offset = component * std::mem::size_of::<i32>();
        let mut out = [0u8; 4];
        out.copy_from_slice(&tmp[offset..offset + 4]);
        i32::from_ne_bytes(out)
    }
}
