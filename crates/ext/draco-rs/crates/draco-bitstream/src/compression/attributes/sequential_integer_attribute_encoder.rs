//! Sequential integer attribute encoder.
//! Reference: `_ref/draco/src/draco/compression/attributes/sequential_integer_attribute_encoder.h|cc`.

use crate::compression::attributes::prediction_schemes::prediction_scheme_encoder_factory::{
    create_prediction_scheme_for_encoder, get_prediction_method_from_options,
};
use crate::compression::attributes::prediction_schemes::prediction_scheme_encoder_interface::PredictionSchemeTypedEncoderInterface;
use crate::compression::attributes::prediction_schemes::prediction_scheme_wrap_encoding_transform::PredictionSchemeWrapEncodingTransform;
use crate::compression::attributes::sequential_attribute_encoder::{
    SequentialAttributeEncoderBase, SequentialAttributeEncoderInterface,
};
use crate::compression::config::compression_shared::PredictionSchemeMethod;
use crate::compression::entropy::symbol_encoding::{
    encode_symbols, set_symbol_encoding_compression_level,
};
use crate::compression::point_cloud::PointCloudEncoder;
use draco_core::attributes::geometry_attribute::GeometryAttribute;
use draco_core::attributes::geometry_indices::{AttributeValueIndex, PointIndex};
use draco_core::attributes::point_attribute::PointAttribute;
use draco_core::core::bit_utils::convert_signed_ints_to_symbols;
use draco_core::core::draco_types::DataType;
use draco_core::core::encoder_buffer::EncoderBuffer;
use draco_core::core::options::Options;

pub struct SequentialIntegerAttributeEncoder {
    base: SequentialAttributeEncoderBase,
    prediction_scheme: Option<Box<dyn PredictionSchemeTypedEncoderInterface<i32, i32>>>,
}

impl SequentialIntegerAttributeEncoder {
    pub fn new() -> Self {
        Self {
            base: SequentialAttributeEncoderBase::new(),
            prediction_scheme: None,
        }
    }

    fn prepare_portable_attribute(
        &mut self,
        num_entries: usize,
        num_components: i32,
        num_points: i32,
    ) {
        let mut ga = GeometryAttribute::new();
        ga.init(
            self.base.attribute().attribute_type(),
            None,
            num_components as u8,
            DataType::Int32,
            false,
            (num_components as i64)
                * (draco_core::core::draco_types::data_type_length(DataType::Int32) as i64),
            0,
        );
        let mut port_att = PointAttribute::from_geometry_attribute(ga);
        let _ = port_att.reset(num_entries);
        self.base.set_portable_attribute(port_att);
        if num_points > 0 {
            if let Some(portable) = self.base.portable_attribute_mut() {
                portable.set_explicit_mapping(num_points as usize);
            }
        }
    }

    fn prepare_values(&mut self, point_ids: &[PointIndex], num_points: i32) -> bool {
        let (num_components, values) = {
            let attrib = self.base.attribute();
            let num_components = attrib.num_components();
            let mut values = Vec::with_capacity(point_ids.len() * num_components as usize);
            let mut temp = vec![0i32; num_components as usize];
            for &pi in point_ids {
                let att_id = attrib.mapped_index(pi);
                if !attrib.convert_value::<i32>(att_id, num_components as i8, &mut temp) {
                    return false;
                }
                values.extend_from_slice(&temp);
            }
            (num_components, values)
        };
        let num_entries = point_ids.len();
        self.prepare_portable_attribute(num_entries, num_components as i32, num_points);
        if let Some(portable) = self.base.portable_attribute_mut() {
            let entry_stride = num_components as usize;
            for (entry_index, chunk) in values.chunks(entry_stride).enumerate() {
                let bytes = unsafe {
                    std::slice::from_raw_parts(
                        chunk.as_ptr() as *const u8,
                        entry_stride * std::mem::size_of::<i32>(),
                    )
                };
                portable.set_attribute_value_bytes(
                    AttributeValueIndex::from(entry_index as u32),
                    bytes,
                );
            }
        }
        true
    }

    fn load_portable_values(&self, num_values: usize) -> Option<Vec<i32>> {
        let portable = self.base.portable_attribute()?;
        let buffer = portable.buffer()?;
        let byte_len = num_values.checked_mul(std::mem::size_of::<i32>())?;
        if buffer.borrow().data_size() < byte_len {
            return None;
        }
        let mut raw = vec![0u8; byte_len];
        buffer.borrow().read(0, &mut raw);
        let mut values = vec![0i32; num_values];
        for (i, chunk) in raw.chunks_exact(std::mem::size_of::<i32>()).enumerate() {
            values[i] = i32::from_ne_bytes(chunk.try_into().ok()?);
        }
        Some(values)
    }
}

impl Default for SequentialIntegerAttributeEncoder {
    fn default() -> Self {
        Self::new()
    }
}

impl SequentialAttributeEncoderInterface for SequentialIntegerAttributeEncoder {
    fn base(&self) -> &SequentialAttributeEncoderBase {
        &self.base
    }

    fn base_mut(&mut self) -> &mut SequentialAttributeEncoderBase {
        &mut self.base
    }

    fn get_unique_id(&self) -> u8 {
        crate::compression::config::compression_shared::SequentialAttributeEncoderType::SequentialAttributeEncoderInteger
            as u8
    }

    fn init(&mut self, encoder: &mut dyn PointCloudEncoder, attribute_id: i32) -> bool {
        if !self.base.init(encoder, attribute_id) {
            return false;
        }
        if self.get_unique_id()
            == crate::compression::config::compression_shared::SequentialAttributeEncoderType::SequentialAttributeEncoderInteger
                as u8
        {
            match self.base.attribute().data_type() {
                DataType::Int8
                | DataType::Uint8
                | DataType::Int16
                | DataType::Uint16
                | DataType::Int32
                | DataType::Uint32 => {}
                _ => return false,
            }
        }
        let prediction_scheme_method =
            get_prediction_method_from_options(attribute_id, encoder.options());
        let scheme = create_prediction_scheme_for_encoder(
            prediction_scheme_method,
            attribute_id,
            encoder,
            PredictionSchemeWrapEncodingTransform::default(),
        );
        if let Some(mut scheme) = scheme {
            if !self.base.init_prediction_scheme(scheme.as_mut()) {
                self.prediction_scheme = None;
            } else {
                self.prediction_scheme = Some(scheme);
            }
        } else {
            self.prediction_scheme = None;
        }
        true
    }

    fn transform_attribute_to_portable_format(&mut self, point_ids: &[PointIndex]) -> bool {
        let num_points = self
            .base
            .point_cloud()
            .map(|pc| pc.num_points() as i32)
            .unwrap_or(0);
        if !self.prepare_values(point_ids, num_points) {
            return false;
        }
        if self.base.is_parent_encoder() {
            let num_points_usize = num_points.max(0) as usize;
            let mapping_entries = {
                let orig_att = self.base.attribute();
                let mut value_to_value_map =
                    draco_core::core::draco_index_type_vector::IndexTypeVector::<
                        AttributeValueIndex,
                        AttributeValueIndex,
                    >::with_size_value(
                        orig_att.size(), AttributeValueIndex::from(0u32)
                    );
                for (i, &pi) in point_ids.iter().enumerate() {
                    value_to_value_map[orig_att.mapped_index(pi)] =
                        AttributeValueIndex::from(i as u32);
                }
                let mut entries = Vec::with_capacity(num_points_usize);
                for i in 0..num_points_usize {
                    let pi = PointIndex::from(i as u32);
                    entries.push(value_to_value_map[orig_att.mapped_index(pi)]);
                }
                entries
            };
            if let Some(portable) = self.base.portable_attribute_mut() {
                if portable.is_mapping_identity() {
                    portable.set_explicit_mapping(num_points_usize);
                }
                for (i, mapped) in mapping_entries.into_iter().enumerate() {
                    let pi = PointIndex::from(i as u32);
                    portable.set_point_map_entry(pi, mapped);
                }
            }
        }
        true
    }

    fn encode_values(&mut self, point_ids: &[PointIndex], out_buffer: &mut EncoderBuffer) -> bool {
        let portable = match self.base.portable_attribute() {
            Some(att) => att,
            None => return false,
        };
        if portable.size() == 0 {
            return true;
        }
        let mut prediction_scheme_method = PredictionSchemeMethod::PredictionNone as i8;
        if let Some(scheme) = self.prediction_scheme.as_mut() {
            if !self
                .base
                .set_prediction_scheme_parent_attributes(scheme.as_mut())
            {
                return false;
            }
            prediction_scheme_method = scheme.get_prediction_method() as i8;
        }
        if !out_buffer.encode(prediction_scheme_method) {
            return false;
        }
        if let Some(scheme) = self.prediction_scheme.as_ref() {
            if !out_buffer.encode(scheme.get_transform_type() as i8) {
                return false;
            }
        }

        let num_components = portable.num_components();
        let num_values = (num_components as usize) * portable.size();
        let portable_data = match self.load_portable_values(num_values) {
            Some(values) => values,
            None => return false,
        };
        let mut encoded_data = vec![0i32; num_values];
        if let Some(scheme) = self.prediction_scheme.as_mut() {
            if !scheme.compute_correction_values(
                &portable_data,
                &mut encoded_data,
                num_components as i32,
                point_ids,
            ) {
                return false;
            }
        }
        let input = if self.prediction_scheme.is_some() {
            &encoded_data
        } else {
            &portable_data
        };
        let mut symbols = vec![0u32; num_values];
        if self.prediction_scheme.is_none()
            || !self
                .prediction_scheme
                .as_ref()
                .unwrap()
                .are_corrections_positive()
        {
            convert_signed_ints_to_symbols(input, &mut symbols);
        } else {
            for i in 0..num_values {
                symbols[i] = input[i] as u32;
            }
        }

        let use_compression = self
            .base
            .options()
            .map(|options| options.get_global_bool("use_built_in_attribute_compression", true))
            .unwrap_or(true);
        if use_compression {
            if !out_buffer.encode(1u8) {
                return false;
            }
            let mut symbol_options = Options::new();
            if let Some(options) = self.base.options() {
                let _ = set_symbol_encoding_compression_level(
                    &mut symbol_options,
                    10 - options.get_speed(),
                );
            }
            if !encode_symbols(
                &symbols,
                num_values as i32,
                num_components as i32,
                Some(&symbol_options),
                out_buffer,
            ) {
                return false;
            }
        } else {
            let mut masked_value = 0u32;
            for v in &symbols {
                masked_value |= *v;
            }
            let mut value_msb_pos = 0i32;
            if masked_value != 0 {
                value_msb_pos = draco_core::core::bit_utils::most_significant_bit(masked_value);
            }
            let num_bytes = 1 + (value_msb_pos / 8);
            if !out_buffer.encode(0u8) {
                return false;
            }
            if !out_buffer.encode(num_bytes as u8) {
                return false;
            }
            if num_bytes as usize == std::mem::size_of::<i32>() {
                let bytes = unsafe {
                    std::slice::from_raw_parts(
                        symbols.as_ptr() as *const u8,
                        num_values * std::mem::size_of::<u32>(),
                    )
                };
                if !out_buffer.encode_bytes(bytes) {
                    return false;
                }
            } else {
                for v in &symbols {
                    let bytes = v.to_le_bytes();
                    if !out_buffer.encode_bytes(&bytes[0..num_bytes as usize]) {
                        return false;
                    }
                }
            }
        }
        if let Some(scheme) = &self.prediction_scheme {
            scheme.encode_prediction_data(out_buffer)
        } else {
            true
        }
    }
}
