//! kD-tree attributes decoder.
//! Reference: `_ref/draco/src/draco/compression/attributes/kd_tree_attributes_decoder.h|cc`.
//!
//! Decodes attributes encoded by KdTreeAttributesEncoder.

use crate::compression::attributes::attributes_decoder::{
    AttributesDecoder, AttributesDecoderBase,
};
use crate::compression::attributes::attributes_decoder_interface::AttributesDecoderInterface;
use crate::compression::attributes::kd_tree_attributes_shared::KdTreeAttributesEncodingMethod;
use crate::compression::point_cloud::algorithms::dynamic_integer_points_kd_tree_decoder::DynamicIntegerPointsKdTreeDecoder;
use crate::compression::point_cloud::algorithms::float_points_tree_decoder::FloatPointsTreeDecoder;
use crate::compression::point_cloud::algorithms::quantize_points_3::PointOutput;
use draco_core::attributes::attribute_quantization_transform::AttributeQuantizationTransform;
use draco_core::attributes::attribute_transform::AttributeTransform;
use draco_core::attributes::geometry_attribute::GeometryAttribute;
use draco_core::attributes::geometry_indices::{AttributeValueIndex, PointIndex};
use draco_core::attributes::point_attribute::PointAttribute;
use draco_core::core::decoder_buffer::DecoderBuffer;
use draco_core::core::draco_types::{data_type_length, DataType};
use draco_core::core::varint_decoding::decode_varint;
use draco_core::point_cloud::point_cloud::PointCloud;

/// Attribute tuple used by the output iterator.
struct AttributeTuple {
    attribute: *mut PointAttribute,
    offset_dimensionality: usize,
    data_size: usize,
    num_components: usize,
}

/// Output iterator that writes decoded vectors into PointAttribute buffers.
struct PointAttributeVectorOutput {
    attributes: Vec<AttributeTuple>,
    point_id: u32,
    scratch: Vec<u8>,
}

impl PointAttributeVectorOutput {
    fn new(attributes: Vec<AttributeTuple>) -> Self {
        let mut required_decode_bytes = 0usize;
        for att in &attributes {
            required_decode_bytes = required_decode_bytes.max(att.data_size * att.num_components);
        }
        Self {
            attributes,
            point_id: 0,
            scratch: vec![0u8; required_decode_bytes],
        }
    }

    fn write_u32(&mut self, point: &[u32]) {
        for att in &self.attributes {
            let attribute = unsafe { &mut *att.attribute };
            let avi = attribute.mapped_index(PointIndex::from(self.point_id));
            if avi.value() >= attribute.size() as u32 {
                return;
            }
            let offset = att.offset_dimensionality;
            if point.len() < offset + att.num_components {
                return;
            }
            let data_source = &point[offset..offset + att.num_components];
            if att.data_size < 4 {
                let mut cursor = 0usize;
                for v in data_source {
                    let bytes = v.to_ne_bytes();
                    self.scratch[cursor..cursor + att.data_size]
                        .copy_from_slice(&bytes[0..att.data_size]);
                    cursor += att.data_size;
                }
                attribute.set_attribute_value_bytes(avi, &self.scratch[0..cursor]);
            } else {
                let bytes = unsafe {
                    std::slice::from_raw_parts(
                        data_source.as_ptr() as *const u8,
                        att.num_components * std::mem::size_of::<u32>(),
                    )
                };
                attribute.set_attribute_value_bytes(avi, bytes);
            }
        }
        self.point_id += 1;
    }

    fn write_f32(&mut self, point: &[f32]) {
        for att in &self.attributes {
            let attribute = unsafe { &mut *att.attribute };
            let avi = attribute.mapped_index(PointIndex::from(self.point_id));
            if avi.value() >= attribute.size() as u32 {
                return;
            }
            let offset = att.offset_dimensionality;
            if point.len() < offset + att.num_components {
                return;
            }
            let data_source = &point[offset..offset + att.num_components];
            let bytes = unsafe {
                std::slice::from_raw_parts(
                    data_source.as_ptr() as *const u8,
                    att.num_components * std::mem::size_of::<f32>(),
                )
            };
            attribute.set_attribute_value_bytes(avi, bytes);
        }
        self.point_id += 1;
    }
}

impl PointOutput<u32> for PointAttributeVectorOutput {
    fn write_point(&mut self, point: &[u32]) {
        self.write_u32(point);
    }
}

impl PointOutput<f32> for PointAttributeVectorOutput {
    fn write_point(&mut self, point: &[f32]) {
        self.write_f32(point);
    }
}

/// Decodes all attributes using kD-tree compression.
pub struct KdTreeAttributesDecoder {
    base: AttributesDecoderBase,
    attribute_quantization_transforms: Vec<AttributeQuantizationTransform>,
    quantized_portable_attributes: Vec<PointAttribute>,
    min_signed_values: Vec<i32>,
}

impl KdTreeAttributesDecoder {
    pub fn new() -> Self {
        Self {
            base: AttributesDecoderBase::new(),
            attribute_quantization_transforms: Vec::new(),
            quantized_portable_attributes: Vec::new(),
            min_signed_values: Vec::new(),
        }
    }

    fn decode_points<O: PointOutput<u32>>(
        &mut self,
        total_dimensionality: usize,
        num_points: usize,
        in_buffer: &mut DecoderBuffer,
        out: &mut O,
        compression_level: u8,
    ) -> bool {
        let max_points = num_points as u32;
        match compression_level {
            0 => {
                let mut dec =
                    DynamicIntegerPointsKdTreeDecoder::<0>::new(total_dimensionality as u32);
                dec.decode_points(in_buffer, out, max_points)
                    && dec.num_decoded_points() == max_points
            }
            1 => {
                let mut dec =
                    DynamicIntegerPointsKdTreeDecoder::<1>::new(total_dimensionality as u32);
                dec.decode_points(in_buffer, out, max_points)
                    && dec.num_decoded_points() == max_points
            }
            2 => {
                let mut dec =
                    DynamicIntegerPointsKdTreeDecoder::<2>::new(total_dimensionality as u32);
                dec.decode_points(in_buffer, out, max_points)
                    && dec.num_decoded_points() == max_points
            }
            3 => {
                let mut dec =
                    DynamicIntegerPointsKdTreeDecoder::<3>::new(total_dimensionality as u32);
                dec.decode_points(in_buffer, out, max_points)
                    && dec.num_decoded_points() == max_points
            }
            4 => {
                let mut dec =
                    DynamicIntegerPointsKdTreeDecoder::<4>::new(total_dimensionality as u32);
                dec.decode_points(in_buffer, out, max_points)
                    && dec.num_decoded_points() == max_points
            }
            5 => {
                let mut dec =
                    DynamicIntegerPointsKdTreeDecoder::<5>::new(total_dimensionality as u32);
                dec.decode_points(in_buffer, out, max_points)
                    && dec.num_decoded_points() == max_points
            }
            6 => {
                let mut dec =
                    DynamicIntegerPointsKdTreeDecoder::<6>::new(total_dimensionality as u32);
                dec.decode_points(in_buffer, out, max_points)
                    && dec.num_decoded_points() == max_points
            }
            _ => false,
        }
    }

    fn transform_attribute_back_to_signed_i32(
        &self,
        att: &PointAttribute,
        num_processed_signed_components: usize,
    ) -> bool {
        let num_components = att.num_components() as usize;
        let mut unsigned_vals = vec![0u32; num_components];
        let mut signed_vals = vec![0i32; num_components];
        let mut temp = vec![0u8; num_components * std::mem::size_of::<i32>()];

        for i in 0..att.size() {
            let avi = AttributeValueIndex::from(i as u32);
            att.get_value_bytes(avi, &mut temp);
            for c in 0..num_components {
                let offset = c * 4;
                let mut buf = [0u8; 4];
                buf.copy_from_slice(&temp[offset..offset + 4]);
                unsigned_vals[c] = u32::from_ne_bytes(buf);
                if unsigned_vals[c] > i32::MAX as u32 {
                    return false;
                }
                signed_vals[c] = unsigned_vals[c] as i32
                    + self.min_signed_values[num_processed_signed_components + c];
            }
            let bytes = unsafe {
                std::slice::from_raw_parts(
                    signed_vals.as_ptr() as *const u8,
                    num_components * std::mem::size_of::<i32>(),
                )
            };
            att.set_attribute_value_bytes(avi, bytes);
        }
        true
    }

    fn transform_attribute_back_to_signed_i16(
        &self,
        att: &PointAttribute,
        num_processed_signed_components: usize,
    ) -> bool {
        let num_components = att.num_components() as usize;
        let mut unsigned_vals = vec![0u32; num_components];
        let mut signed_vals = vec![0i16; num_components];
        let mut temp = vec![0u8; num_components * std::mem::size_of::<i16>()];

        for i in 0..att.size() {
            let avi = AttributeValueIndex::from(i as u32);
            att.get_value_bytes(avi, &mut temp);
            for c in 0..num_components {
                let offset = c * 2;
                let mut buf = [0u8; 2];
                buf.copy_from_slice(&temp[offset..offset + 2]);
                unsigned_vals[c] = u16::from_ne_bytes(buf) as u32;
                if unsigned_vals[c] > i32::MAX as u32 {
                    return false;
                }
                let signed = unsigned_vals[c] as i32
                    + self.min_signed_values[num_processed_signed_components + c];
                signed_vals[c] = signed as i16;
            }
            let bytes = unsafe {
                std::slice::from_raw_parts(
                    signed_vals.as_ptr() as *const u8,
                    num_components * std::mem::size_of::<i16>(),
                )
            };
            att.set_attribute_value_bytes(avi, bytes);
        }
        true
    }

    fn transform_attribute_back_to_signed_i8(
        &self,
        att: &PointAttribute,
        num_processed_signed_components: usize,
    ) -> bool {
        let num_components = att.num_components() as usize;
        let mut unsigned_vals = vec![0u32; num_components];
        let mut signed_vals = vec![0i8; num_components];
        let mut temp = vec![0u8; num_components * std::mem::size_of::<i8>()];

        for i in 0..att.size() {
            let avi = AttributeValueIndex::from(i as u32);
            att.get_value_bytes(avi, &mut temp);
            for c in 0..num_components {
                let offset = c;
                unsigned_vals[c] = temp[offset] as u32;
                if unsigned_vals[c] > i32::MAX as u32 {
                    return false;
                }
                let signed = unsigned_vals[c] as i32
                    + self.min_signed_values[num_processed_signed_components + c];
                signed_vals[c] = signed as i8;
            }
            let bytes = unsafe {
                std::slice::from_raw_parts(
                    signed_vals.as_ptr() as *const u8,
                    num_components * std::mem::size_of::<i8>(),
                )
            };
            att.set_attribute_value_bytes(avi, bytes);
        }
        true
    }
}

impl Default for KdTreeAttributesDecoder {
    fn default() -> Self {
        Self::new()
    }
}

impl AttributesDecoderInterface for KdTreeAttributesDecoder {
    fn init(
        &mut self,
        decoder: &mut dyn crate::compression::point_cloud::PointCloudDecoder,
        pc: &mut draco_core::point_cloud::point_cloud::PointCloud,
    ) -> bool {
        self.base.init(decoder, pc)
    }

    fn decode_attributes_decoder_data(&mut self, in_buffer: &mut DecoderBuffer) -> bool {
        self.base.decode_attributes_decoder_data(in_buffer)
    }

    fn decode_attributes(&mut self, in_buffer: &mut DecoderBuffer) -> bool {
        AttributesDecoder::decode_attributes(self, in_buffer)
    }

    fn get_attribute_id(&self, i: i32) -> i32 {
        self.base.get_attribute_id(i)
    }

    fn get_num_attributes(&self) -> i32 {
        self.base.get_num_attributes()
    }

    fn get_decoder(&self) -> Option<&dyn crate::compression::point_cloud::PointCloudDecoder> {
        self.base.decoder()
    }
}

impl AttributesDecoder for KdTreeAttributesDecoder {
    fn base(&self) -> &AttributesDecoderBase {
        &self.base
    }

    fn base_mut(&mut self) -> &mut AttributesDecoderBase {
        &mut self.base
    }

    fn decode_portable_attributes(&mut self, in_buffer: &mut DecoderBuffer) -> bool {
        if in_buffer.bitstream_version()
            < crate::compression::config::compression_shared::bitstream_version(2, 3)
        {
            // Older bitstream decodes everything in DecodeDataNeededByPortableTransforms().
            return true;
        }

        let mut compression_level: u8 = 0;
        if !in_buffer.decode(&mut compression_level) {
            return false;
        }

        let num_points = match self.base.decoder() {
            Some(dec) => dec
                .point_cloud()
                .map(|pc| pc.num_points() as usize)
                .unwrap_or(0),
            None => return false,
        };

        let num_attributes = self.base.get_num_attributes();
        let mut total_dimensionality = 0usize;
        let mut atts: Vec<AttributeTuple> = Vec::with_capacity(num_attributes as usize);

        let pc_ptr: *mut PointCloud = match self
            .base
            .decoder_mut()
            .and_then(|dec| dec.point_cloud_mut())
        {
            Some(pc) => pc as *mut _,
            None => return false,
        };

        unsafe {
            let pc = &mut *pc_ptr;
            self.min_signed_values.clear();
            self.quantized_portable_attributes.clear();
            self.attribute_quantization_transforms.clear();

            for i in 0..num_attributes {
                let att_id = self.base.get_attribute_id(i);
                let att = match pc.attribute_mut(att_id) {
                    Some(att) => att,
                    None => return false,
                };
                att.reset(num_points);
                att.set_identity_mapping();

                let mut target_att_ptr: *mut PointAttribute = att as *mut PointAttribute;
                if matches!(
                    att.data_type(),
                    DataType::Uint32 | DataType::Uint16 | DataType::Uint8
                ) {
                    // Decode directly into the attribute.
                } else if matches!(
                    att.data_type(),
                    DataType::Int32 | DataType::Int16 | DataType::Int8
                ) {
                    // Prepare storage for min signed values per component.
                    for _ in 0..att.num_components() {
                        self.min_signed_values.push(0);
                    }
                } else if att.data_type() == DataType::Float32 {
                    // Create portable storage for quantized values.
                    let num_components = att.num_components() as i32;
                    let mut ga = GeometryAttribute::new();
                    ga.init(
                        att.attribute_type(),
                        None,
                        num_components as u8,
                        DataType::Uint32,
                        false,
                        (num_components as i64) * (data_type_length(DataType::Uint32) as i64),
                        0,
                    );
                    let mut port_att = PointAttribute::from_geometry_attribute(ga);
                    port_att.set_identity_mapping();
                    port_att.reset(num_points);
                    self.quantized_portable_attributes.push(port_att);
                    let last = self.quantized_portable_attributes.len() - 1;
                    target_att_ptr =
                        &mut self.quantized_portable_attributes[last] as *mut PointAttribute;
                } else {
                    return false;
                }

                let target_att = &mut *target_att_ptr;
                let data_size = data_type_length(target_att.data_type()) as usize;
                let num_components = target_att.num_components() as usize;
                atts.push(AttributeTuple {
                    attribute: target_att_ptr,
                    offset_dimensionality: total_dimensionality,
                    data_size,
                    num_components,
                });
                total_dimensionality += num_components;
            }
        }

        let mut out_it = PointAttributeVectorOutput::new(atts);
        self.decode_points(
            total_dimensionality,
            num_points,
            in_buffer,
            &mut out_it,
            compression_level,
        )
    }

    fn decode_data_needed_by_portable_transforms(&mut self, in_buffer: &mut DecoderBuffer) -> bool {
        if in_buffer.bitstream_version()
            >= crate::compression::config::compression_shared::bitstream_version(2, 3)
        {
            // Decode quantization data for float attributes.
            let mut min_value: Vec<f32> = Vec::new();
            for i in 0..self.base.get_num_attributes() {
                let att_id = self.base.get_attribute_id(i);
                let att = match self
                    .base
                    .decoder()
                    .and_then(|dec| dec.point_cloud().and_then(|pc| pc.attribute(att_id)))
                {
                    Some(att) => att,
                    None => return false,
                };
                if att.data_type() == DataType::Float32 {
                    let num_components = att.num_components() as usize;
                    min_value.resize(num_components, 0.0);
                    let bytes = unsafe {
                        std::slice::from_raw_parts_mut(
                            min_value.as_mut_ptr() as *mut u8,
                            num_components * std::mem::size_of::<f32>(),
                        )
                    };
                    if !in_buffer.decode_bytes(bytes) {
                        return false;
                    }
                    let mut max_value_dif: f32 = 0.0;
                    if !in_buffer.decode(&mut max_value_dif) {
                        return false;
                    }
                    let mut quantization_bits: u8 = 0;
                    if !in_buffer.decode(&mut quantization_bits) || quantization_bits > 31 {
                        return false;
                    }
                    let mut transform = AttributeQuantizationTransform::new();
                    if !transform.set_parameters(
                        quantization_bits as i32,
                        &min_value,
                        num_components,
                        max_value_dif,
                    ) {
                        return false;
                    }
                    let idx = self.attribute_quantization_transforms.len();
                    if idx >= self.quantized_portable_attributes.len() {
                        return false;
                    }
                    if !transform
                        .transfer_to_attribute(&mut self.quantized_portable_attributes[idx])
                    {
                        return false;
                    }
                    self.attribute_quantization_transforms.push(transform);
                }
            }

            // Decode min signed values.
            for i in 0..self.min_signed_values.len() {
                let mut val: i32 = 0;
                if !decode_varint(&mut val, in_buffer) {
                    return false;
                }
                self.min_signed_values[i] = val;
            }
            return true;
        }

        // Backwards compatibility path (< 2.3).
        let attribute_count = self.base.get_num_attributes();
        let mut total_dimensionality = 0usize;
        let mut atts: Vec<AttributeTuple> = Vec::with_capacity(attribute_count as usize);

        let pc_ptr: *mut PointCloud = match self
            .base
            .decoder_mut()
            .and_then(|dec| dec.point_cloud_mut())
        {
            Some(pc) => pc as *mut _,
            None => return false,
        };
        unsafe {
            let pc = &mut *pc_ptr;
            for i in 0..attribute_count {
                let att_id = self.base.get_attribute_id(i);
                let att = match pc.attribute_mut(att_id) {
                    Some(att) => att,
                    None => return false,
                };
                let data_size = data_type_length(att.data_type()) as usize;
                if data_size > 4 {
                    return false;
                }
                let num_components = att.num_components() as usize;
                atts.push(AttributeTuple {
                    attribute: att as *mut PointAttribute,
                    offset_dimensionality: total_dimensionality,
                    data_size,
                    num_components,
                });
                total_dimensionality += num_components;
            }

            // Prepare for decoding.
            if attribute_count > 0 {
                let att_id = self.base.get_attribute_id(0);
                if let Some(att) = pc.attribute_mut(att_id) {
                    att.set_identity_mapping();
                }
            }
        }

        let mut method: u8 = 0;
        if !in_buffer.decode(&mut method) {
            return false;
        }

        if method == KdTreeAttributesEncodingMethod::KdTreeQuantizationEncoding as u8 {
            if atts.len() != 1 || atts[0].num_components != 3 {
                return false;
            }
            let mut compression_level: u8 = 0;
            if !in_buffer.decode(&mut compression_level) {
                return false;
            }
            let mut num_points: u32 = 0;
            if !in_buffer.decode(&mut num_points) {
                return false;
            }
            unsafe {
                let pc = &mut *pc_ptr;
                let att_id = self.base.get_attribute_id(0);
                if let Some(att) = pc.attribute_mut(att_id) {
                    att.reset(num_points as usize);
                    att.set_identity_mapping();
                }
            }
            let mut decoder = FloatPointsTreeDecoder::new();
            decoder.set_num_points_from_header(num_points);
            let mut out_it = PointAttributeVectorOutput::new(atts);
            return decoder.decode_point_cloud(in_buffer, &mut out_it);
        }

        if method == KdTreeAttributesEncodingMethod::KdTreeIntegerEncoding as u8 {
            let mut compression_level: u8 = 0;
            if !in_buffer.decode(&mut compression_level) {
                return false;
            }
            if compression_level > 6 {
                return false;
            }
            let mut num_points: u32 = 0;
            if !in_buffer.decode(&mut num_points) {
                return false;
            }
            unsafe {
                let pc = &mut *pc_ptr;
                for i in 0..attribute_count {
                    let att_id = self.base.get_attribute_id(i);
                    if let Some(att) = pc.attribute_mut(att_id) {
                        att.reset(num_points as usize);
                        att.set_identity_mapping();
                    }
                }
            }
            let mut out_it = PointAttributeVectorOutput::new(atts);
            return self.decode_points(
                total_dimensionality,
                num_points as usize,
                in_buffer,
                &mut out_it,
                compression_level,
            );
        }

        false
    }

    fn transform_attributes_to_original_format(&mut self) -> bool {
        if self.quantized_portable_attributes.is_empty() && self.min_signed_values.is_empty() {
            return true;
        }
        let mut num_processed_quantized_attributes = 0usize;
        let mut num_processed_signed_components = 0usize;

        let pc_ptr: *mut PointCloud = match self
            .base
            .decoder_mut()
            .and_then(|dec| dec.point_cloud_mut())
        {
            Some(pc) => pc as *mut _,
            None => return false,
        };

        unsafe {
            let pc = &mut *pc_ptr;
            for i in 0..self.base.get_num_attributes() {
                let att_id = self.base.get_attribute_id(i);
                let att = match pc.attribute_mut(att_id) {
                    Some(att) => att,
                    None => return false,
                };

                if matches!(
                    att.data_type(),
                    DataType::Int32 | DataType::Int16 | DataType::Int8
                ) {
                    let ok = if att.data_type() == DataType::Int32 {
                        self.transform_attribute_back_to_signed_i32(
                            att,
                            num_processed_signed_components,
                        )
                    } else if att.data_type() == DataType::Int16 {
                        self.transform_attribute_back_to_signed_i16(
                            att,
                            num_processed_signed_components,
                        )
                    } else {
                        self.transform_attribute_back_to_signed_i8(
                            att,
                            num_processed_signed_components,
                        )
                    };
                    if !ok {
                        return false;
                    }
                    num_processed_signed_components += att.num_components() as usize;
                } else if att.data_type() == DataType::Float32 {
                    let src_att =
                        &self.quantized_portable_attributes[num_processed_quantized_attributes];
                    let transform =
                        &self.attribute_quantization_transforms[num_processed_quantized_attributes];
                    num_processed_quantized_attributes += 1;

                    if let Some(decoder) = self.base.decoder() {
                        if let Some(opts) = decoder.options() {
                            if opts.get_attribute_bool(
                                &att.attribute_type(),
                                "skip_attribute_transform",
                                false,
                            ) {
                                att.copy_from(src_att);
                                continue;
                            }
                        }
                    }

                    if !transform.inverse_transform_attribute(src_att, att) {
                        return false;
                    }
                }
            }
        }
        true
    }
}
