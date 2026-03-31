//! kD-tree attributes encoder.
//! Reference: `_ref/draco/src/draco/compression/attributes/kd_tree_attributes_encoder.h|cc`.
//!
//! Encodes all point attributes using a single kD-tree encoder.

use crate::compression::attributes::attributes_encoder::{
    AttributesEncoderBase, AttributesEncoderInterface,
};
use crate::compression::attributes::point_d_vector::PointDVector;
use crate::compression::config::compression_shared::AttributeEncoderType;
use crate::compression::point_cloud::algorithms::dynamic_integer_points_kd_tree_encoder::DynamicIntegerPointsKdTreeEncoder;
use draco_core::attributes::attribute_quantization_transform::AttributeQuantizationTransform;
use draco_core::attributes::attribute_transform::AttributeTransform;
use draco_core::attributes::geometry_indices::{AttributeValueIndex, PointIndex};
use draco_core::attributes::point_attribute::PointAttribute;
use draco_core::core::bit_utils::most_significant_bit;
use draco_core::core::draco_types::DataType;
use draco_core::core::encoder_buffer::EncoderBuffer;
use draco_core::core::varint_encoding::encode_varint;

/// Encodes all attributes of a point cloud using kD-tree compression.
pub struct KdTreeAttributesEncoder {
    base: AttributesEncoderBase,
    attribute_quantization_transforms: Vec<AttributeQuantizationTransform>,
    min_signed_values: Vec<i32>,
    quantized_portable_attributes: Vec<PointAttribute>,
    num_components: i32,
}

impl KdTreeAttributesEncoder {
    pub fn new() -> Self {
        Self {
            base: AttributesEncoderBase::new(),
            attribute_quantization_transforms: Vec::new(),
            min_signed_values: Vec::new(),
            quantized_portable_attributes: Vec::new(),
            num_components: 0,
        }
    }

    pub fn with_attribute_id(att_id: i32) -> Self {
        let mut enc = Self::new();
        enc.base.add_attribute_id(att_id);
        enc
    }
}

impl Default for KdTreeAttributesEncoder {
    fn default() -> Self {
        Self::new()
    }
}

impl AttributesEncoderInterface for KdTreeAttributesEncoder {
    fn base(&self) -> &AttributesEncoderBase {
        &self.base
    }

    fn base_mut(&mut self) -> &mut AttributesEncoderBase {
        &mut self.base
    }

    fn get_unique_id(&self) -> u8 {
        AttributeEncoderType::KdTreeAttributeEncoder as u8
    }

    fn transform_attributes_to_portable_format(&mut self) -> bool {
        let options = match self.base.options() {
            Some(options) => options,
            None => return false,
        };
        let pc = match self.base.point_cloud() {
            Some(pc) => pc,
            None => return false,
        };
        let num_points = pc.num_points() as usize;

        // Compute total dimensionality across all attributes.
        let mut num_components = 0i32;
        for i in 0..self.base.num_attributes() {
            let att_id = self.base.get_attribute_id(i as i32);
            let att = match pc.attribute(att_id) {
                Some(att) => att,
                None => return false,
            };
            num_components += att.num_components() as i32;
        }
        self.num_components = num_components;

        // Quantize floats and track min signed values for signed integers.
        self.attribute_quantization_transforms.clear();
        self.quantized_portable_attributes.clear();
        self.min_signed_values.clear();

        for i in 0..self.base.num_attributes() {
            let att_id = self.base.get_attribute_id(i as i32);
            let att = match pc.attribute(att_id) {
                Some(att) => att,
                None => return false,
            };
            match att.data_type() {
                DataType::Float32 => {
                    // Quantization path for float attributes.
                    let mut transform = AttributeQuantizationTransform::new();
                    let quantization_bits =
                        options.get_attribute_int(&att_id, "quantization_bits", -1);
                    if quantization_bits < 1 {
                        return false;
                    }
                    if options.is_attribute_option_set(&att_id, "quantization_origin")
                        && options.is_attribute_option_set(&att_id, "quantization_range")
                    {
                        let mut origin = vec![0.0f32; att.num_components() as usize];
                        if !options.get_attribute_vector(
                            &att_id,
                            "quantization_origin",
                            att.num_components() as i32,
                            &mut origin,
                        ) {
                            return false;
                        }
                        let range = options.get_attribute_float(&att_id, "quantization_range", 1.0);
                        if !transform.set_parameters(
                            quantization_bits,
                            &origin,
                            att.num_components() as usize,
                            range,
                        ) {
                            return false;
                        }
                    } else if !transform.compute_parameters(att, quantization_bits) {
                        return false;
                    }

                    let mut portable = transform.init_transformed_attribute(att, num_points as i32);
                    if !transform.transform_attribute(att, &[], &mut portable) {
                        return false;
                    }
                    self.attribute_quantization_transforms.push(transform);
                    self.quantized_portable_attributes.push(portable);
                }
                DataType::Int32 | DataType::Int16 | DataType::Int8 => {
                    // For signed types, capture min value per component.
                    let mut min_value = vec![i32::MAX; att.num_components() as usize];
                    let mut act_value = vec![0i32; att.num_components() as usize];
                    for avi in 0..att.size() {
                        let idx = AttributeValueIndex::from(avi as u32);
                        if !att.convert_value::<i32>(
                            idx,
                            att.num_components() as i8,
                            &mut act_value,
                        ) {
                            return false;
                        }
                        for c in 0..att.num_components() as usize {
                            if min_value[c] > act_value[c] {
                                min_value[c] = act_value[c];
                            }
                        }
                    }
                    for c in 0..att.num_components() as usize {
                        self.min_signed_values.push(min_value[c]);
                    }
                }
                _ => {}
            }
        }
        true
    }

    fn encode_data_needed_by_portable_transforms(
        &mut self,
        out_buffer: &mut EncoderBuffer,
    ) -> bool {
        // Encode quantization parameters for float attributes.
        for transform in &self.attribute_quantization_transforms {
            if !transform.encode_parameters(out_buffer) {
                return false;
            }
        }

        // Encode min signed values for signed integer attributes.
        for &val in &self.min_signed_values {
            if !encode_varint(val, out_buffer) {
                return false;
            }
        }
        true
    }

    fn encode_portable_attributes(&mut self, out_buffer: &mut EncoderBuffer) -> bool {
        let options = match self.base.options() {
            Some(options) => options,
            None => return false,
        };
        let pc = match self.base.point_cloud() {
            Some(pc) => pc,
            None => return false,
        };

        // Compression level derived from speed (clamped to <= 6).
        let mut compression_level = 10 - options.get_speed();
        if compression_level > 6 {
            compression_level = 6;
        }
        if compression_level < 0 {
            compression_level = 0;
        }
        if compression_level == 6 && self.num_components > 15 {
            compression_level = 5;
        }
        if !out_buffer.encode(compression_level as u8) {
            return false;
        }

        let num_points = pc.num_points() as usize;
        let mut point_vector = PointDVector::<u32>::new(num_points, self.num_components as usize);

        let mut num_processed_components = 0usize;
        let mut num_processed_quantized_attributes = 0usize;
        let mut num_processed_signed_components = 0usize;

        for i in 0..self.base.num_attributes() {
            let att_id = self.base.get_attribute_id(i as i32);
            let att = match pc.attribute(att_id) {
                Some(att) => att,
                None => return false,
            };

            let source_att: &PointAttribute = match att.data_type() {
                DataType::Uint32
                | DataType::Uint16
                | DataType::Uint8
                | DataType::Int32
                | DataType::Int16
                | DataType::Int8 => att,
                DataType::Float32 => {
                    let src =
                        &self.quantized_portable_attributes[num_processed_quantized_attributes];
                    num_processed_quantized_attributes += 1;
                    src
                }
                _ => return false,
            };

            if source_att.data_type() == DataType::Uint32 {
                // Directly copy uint32 data.
                for pi in 0..num_points {
                    let avi = source_att.mapped_index(PointIndex::from(pi as u32));
                    let mut bytes = vec![
                        0u8;
                        source_att.num_components() as usize
                            * std::mem::size_of::<u32>()
                    ];
                    source_att.get_value_bytes(avi, &mut bytes);
                    point_vector.copy_attribute_bytes(
                        source_att.num_components() as usize,
                        num_processed_components,
                        pi,
                        bytes.as_ptr(),
                    );
                }
            } else if matches!(
                source_att.data_type(),
                DataType::Int32 | DataType::Int16 | DataType::Int8
            ) {
                // Signed values need to be shifted to unsigned domain.
                let mut signed_point = vec![0i32; source_att.num_components() as usize];
                let mut unsigned_point = vec![0u32; source_att.num_components() as usize];
                for pi in 0..num_points {
                    let avi = source_att.mapped_index(PointIndex::from(pi as u32));
                    if !source_att.convert_value::<i32>(
                        avi,
                        source_att.num_components() as i8,
                        &mut signed_point,
                    ) {
                        return false;
                    }
                    for c in 0..source_att.num_components() as usize {
                        unsigned_point[c] = (signed_point[c]
                            - self.min_signed_values[num_processed_signed_components + c])
                            as u32;
                    }
                    point_vector.copy_attribute_bytes(
                        source_att.num_components() as usize,
                        num_processed_components,
                        pi,
                        unsigned_point.as_ptr() as *const u8,
                    );
                }
                num_processed_signed_components += source_att.num_components() as usize;
            } else {
                // Convert to uint32 values on the fly.
                let mut point = vec![0u32; source_att.num_components() as usize];
                for pi in 0..num_points {
                    let avi = source_att.mapped_index(PointIndex::from(pi as u32));
                    if !source_att.convert_value::<u32>(
                        avi,
                        source_att.num_components() as i8,
                        &mut point,
                    ) {
                        return false;
                    }
                    point_vector.copy_attribute_bytes(
                        source_att.num_components() as usize,
                        num_processed_components,
                        pi,
                        point.as_ptr() as *const u8,
                    );
                }
            }

            num_processed_components += source_att.num_components() as usize;
        }

        // Compute maximum bit length of all coordinates.
        let mut num_bits = 0i32;
        for &value in point_vector.data() {
            if value > 0 {
                let msb = most_significant_bit(value) + 1;
                if msb > num_bits {
                    num_bits = msb;
                }
            }
        }

        // Encode using dynamic integer kD-tree encoder.
        match compression_level {
            6 => {
                let mut enc =
                    DynamicIntegerPointsKdTreeEncoder::<6>::new(self.num_components as u32);
                enc.encode_points(&mut point_vector, num_bits as u32, out_buffer)
            }
            5 => {
                let mut enc =
                    DynamicIntegerPointsKdTreeEncoder::<5>::new(self.num_components as u32);
                enc.encode_points(&mut point_vector, num_bits as u32, out_buffer)
            }
            4 => {
                let mut enc =
                    DynamicIntegerPointsKdTreeEncoder::<4>::new(self.num_components as u32);
                enc.encode_points(&mut point_vector, num_bits as u32, out_buffer)
            }
            3 => {
                let mut enc =
                    DynamicIntegerPointsKdTreeEncoder::<3>::new(self.num_components as u32);
                enc.encode_points(&mut point_vector, num_bits as u32, out_buffer)
            }
            2 => {
                let mut enc =
                    DynamicIntegerPointsKdTreeEncoder::<2>::new(self.num_components as u32);
                enc.encode_points(&mut point_vector, num_bits as u32, out_buffer)
            }
            1 => {
                let mut enc =
                    DynamicIntegerPointsKdTreeEncoder::<1>::new(self.num_components as u32);
                enc.encode_points(&mut point_vector, num_bits as u32, out_buffer)
            }
            0 => {
                let mut enc =
                    DynamicIntegerPointsKdTreeEncoder::<0>::new(self.num_components as u32);
                enc.encode_points(&mut point_vector, num_bits as u32, out_buffer)
            }
            _ => false,
        }
    }
}
