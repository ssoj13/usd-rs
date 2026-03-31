//! Sequential integer attribute decoder.
//! Reference: `_ref/draco/src/draco/compression/attributes/sequential_integer_attribute_decoder.h|cc`.
//!
//! Decodes integer attributes using optional prediction schemes and symbol
//! decoding. Values are stored in a portable int32 attribute and then
//! transformed back to the original attribute storage.

use crate::compression::attributes::prediction_schemes::prediction_scheme_decoder_factory::create_prediction_scheme_for_decoder;
use crate::compression::attributes::prediction_schemes::prediction_scheme_decoder_interface::PredictionSchemeTypedDecoderInterface;
use crate::compression::attributes::prediction_schemes::prediction_scheme_wrap_decoding_transform::PredictionSchemeWrapDecodingTransform;
use crate::compression::attributes::sequential_attribute_decoder::{
    SequentialAttributeDecoderBase, SequentialAttributeDecoderInterface,
};
use crate::compression::config::compression_shared::{
    bitstream_version, PredictionSchemeMethod, PredictionSchemeTransformType,
};
use crate::compression::entropy::symbol_decoding::decode_symbols;
use crate::compression::point_cloud::PointCloudDecoder;
use draco_core::attributes::geometry_attribute::GeometryAttribute;
use draco_core::attributes::geometry_indices::PointIndex;
use draco_core::attributes::point_attribute::PointAttribute;
use draco_core::core::bit_utils::convert_symbols_to_signed_ints;
use draco_core::core::decoder_buffer::DecoderBuffer;
use draco_core::core::draco_types::{data_type_length, DataType};
use num_traits::NumCast;

pub struct SequentialIntegerAttributeDecoder {
    base: SequentialAttributeDecoderBase,
    prediction_scheme: Option<Box<dyn PredictionSchemeTypedDecoderInterface<i32, i32>>>,
}

impl SequentialIntegerAttributeDecoder {
    pub fn new() -> Self {
        Self {
            base: SequentialAttributeDecoderBase::new(),
            prediction_scheme: None,
        }
    }

    fn get_num_value_components(&self) -> i32 {
        self.base.attribute().num_components() as i32
    }

    fn store_portable_values(&mut self, values: &[i32]) -> bool {
        let portable = match self.base.portable_attribute() {
            Some(att) => att,
            None => return false,
        };
        let mut bytes = Vec::with_capacity(std::mem::size_of_val(values));
        for value in values {
            bytes.extend_from_slice(&value.to_ne_bytes());
        }
        let buffer = match portable.buffer() {
            Some(buffer) => buffer,
            None => return false,
        };
        if buffer.borrow().data_size() < bytes.len() {
            return false;
        }
        buffer.borrow_mut().write(0, &bytes);
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

    fn prepare_portable_attribute(&mut self, num_entries: usize, num_components: i32) {
        let mut ga = GeometryAttribute::new();
        ga.init(
            self.base.attribute().attribute_type(),
            None,
            num_components as u8,
            DataType::Int32,
            false,
            (num_components as i64) * (data_type_length(DataType::Int32) as i64),
            0,
        );
        let mut port_att = PointAttribute::from_geometry_attribute(ga);
        port_att.set_identity_mapping();
        let _ = port_att.reset(num_entries);
        port_att.set_unique_id(self.base.attribute().unique_id());
        self.base.set_portable_attribute(port_att);
    }

    fn create_int_prediction_scheme(
        &self,
        method: PredictionSchemeMethod,
        transform_type: PredictionSchemeTransformType,
    ) -> Option<Box<dyn PredictionSchemeTypedDecoderInterface<i32, i32>>> {
        if transform_type != PredictionSchemeTransformType::PredictionTransformWrap {
            return None;
        }
        let decoder = self.base.decoder()?;
        create_prediction_scheme_for_decoder(
            method,
            self.base.attribute_id(),
            decoder,
            PredictionSchemeWrapDecodingTransform::default(),
        )
    }

    fn prediction_method_from_i8(value: i8) -> Option<PredictionSchemeMethod> {
        let method = match value {
            x if x == PredictionSchemeMethod::PredictionNone as i8 => {
                PredictionSchemeMethod::PredictionNone
            }
            x if x == PredictionSchemeMethod::PredictionUndefined as i8 => {
                PredictionSchemeMethod::PredictionUndefined
            }
            x if x == PredictionSchemeMethod::PredictionDifference as i8 => {
                PredictionSchemeMethod::PredictionDifference
            }
            x if x == PredictionSchemeMethod::MeshPredictionParallelogram as i8 => {
                PredictionSchemeMethod::MeshPredictionParallelogram
            }
            x if x == PredictionSchemeMethod::MeshPredictionMultiParallelogram as i8 => {
                PredictionSchemeMethod::MeshPredictionMultiParallelogram
            }
            x if x == PredictionSchemeMethod::MeshPredictionTexCoordsDeprecated as i8 => {
                PredictionSchemeMethod::MeshPredictionTexCoordsDeprecated
            }
            x if x == PredictionSchemeMethod::MeshPredictionConstrainedMultiParallelogram as i8 => {
                PredictionSchemeMethod::MeshPredictionConstrainedMultiParallelogram
            }
            x if x == PredictionSchemeMethod::MeshPredictionTexCoordsPortable as i8 => {
                PredictionSchemeMethod::MeshPredictionTexCoordsPortable
            }
            x if x == PredictionSchemeMethod::MeshPredictionGeometricNormal as i8 => {
                PredictionSchemeMethod::MeshPredictionGeometricNormal
            }
            x if x == PredictionSchemeMethod::NumPredictionSchemes as i8 => {
                PredictionSchemeMethod::NumPredictionSchemes
            }
            _ => return None,
        };
        Some(method)
    }

    fn transform_type_from_i8(value: i8) -> Option<PredictionSchemeTransformType> {
        let t = match value {
            x if x == PredictionSchemeTransformType::PredictionTransformNone as i8 => {
                PredictionSchemeTransformType::PredictionTransformNone
            }
            x if x == PredictionSchemeTransformType::PredictionTransformDelta as i8 => {
                PredictionSchemeTransformType::PredictionTransformDelta
            }
            x if x == PredictionSchemeTransformType::PredictionTransformWrap as i8 => {
                PredictionSchemeTransformType::PredictionTransformWrap
            }
            x if x == PredictionSchemeTransformType::PredictionTransformNormalOctahedron as i8 => {
                PredictionSchemeTransformType::PredictionTransformNormalOctahedron
            }
            x if x
                == PredictionSchemeTransformType::PredictionTransformNormalOctahedronCanonicalized
                    as i8 =>
            {
                PredictionSchemeTransformType::PredictionTransformNormalOctahedronCanonicalized
            }
            x if x == PredictionSchemeTransformType::NumPredictionSchemeTransformTypes as i8 => {
                PredictionSchemeTransformType::NumPredictionSchemeTransformTypes
            }
            _ => return None,
        };
        Some(t)
    }

    fn decode_integer_values(
        &mut self,
        point_ids: &[PointIndex],
        in_buffer: &mut DecoderBuffer,
    ) -> bool {
        let num_components = self.get_num_value_components();
        if num_components <= 0 {
            return false;
        }
        let num_entries = point_ids.len();
        let num_values = num_entries * (num_components as usize);
        self.prepare_portable_attribute(num_entries, num_components);

        let mut compressed: u8 = 0;
        if !in_buffer.decode(&mut compressed) {
            return false;
        }

        let mut symbols = vec![0u32; num_values];
        if compressed > 0 {
            if !decode_symbols(num_values as u32, num_components, in_buffer, &mut symbols) {
                return false;
            }
        } else {
            let mut num_bytes: u8 = 0;
            if !in_buffer.decode(&mut num_bytes) {
                return false;
            }
            let portable = match self.base.portable_attribute() {
                Some(att) => att,
                None => return false,
            };
            let required = (num_bytes as usize) * num_values;
            if let Some(buf) = portable.buffer() {
                if buf.borrow().data_size() < required {
                    return false;
                }
            }
            if in_buffer.remaining_size() < required as i64 {
                return false;
            }
            if num_bytes as usize == std::mem::size_of::<u32>() {
                for i in 0..num_values {
                    let mut value: u32 = 0;
                    if !in_buffer.decode(&mut value) {
                        return false;
                    }
                    symbols[i] = value;
                }
            } else {
                for i in 0..num_values {
                    let mut tmp = [0u8; 4];
                    if !in_buffer.decode_bytes(&mut tmp[0..num_bytes as usize]) {
                        return false;
                    }
                    symbols[i] = u32::from_le_bytes(tmp);
                }
            }
        }

        let mut values = vec![0i32; num_values];
        if num_values > 0
            && (self.prediction_scheme.is_none()
                || !self
                    .prediction_scheme
                    .as_ref()
                    .unwrap()
                    .are_corrections_positive())
        {
            convert_symbols_to_signed_ints(&symbols, &mut values);
        } else {
            for i in 0..num_values {
                values[i] = symbols[i] as i32;
            }
        }

        if let Some(scheme) = &mut self.prediction_scheme {
            if !scheme.decode_prediction_data(in_buffer) {
                return false;
            }
            if num_values > 0 {
                let corr_values = values.clone();
                if !scheme.compute_original_values(
                    &corr_values,
                    &mut values,
                    num_components,
                    point_ids,
                ) {
                    return false;
                }
            }
        }

        self.store_portable_values(&values)
    }

    fn store_values(&mut self, num_values: usize) -> bool {
        match self.base.attribute().data_type() {
            DataType::Uint8 => self.store_typed_values::<u8>(num_values),
            DataType::Int8 => self.store_typed_values::<i8>(num_values),
            DataType::Uint16 => self.store_typed_values::<u16>(num_values),
            DataType::Int16 => self.store_typed_values::<i16>(num_values),
            DataType::Uint32 => self.store_typed_values::<u32>(num_values),
            DataType::Int32 => self.store_typed_values::<i32>(num_values),
            _ => return false,
        }
        true
    }

    fn store_typed_values<T: Copy + Default + NumCast>(&mut self, num_values: usize) {
        let num_components = self.base.attribute().num_components() as usize;
        let entry_size = std::mem::size_of::<T>() * num_components;
        let mut out_vals = vec![T::default(); num_components];
        let portable_data = match self.load_portable_values(num_values * num_components) {
            Some(values) => values,
            None => return,
        };
        let mut val_id = 0usize;
        let mut out_byte_pos = 0i64;
        for _ in 0..num_values {
            for c in 0..num_components {
                out_vals[c] = NumCast::from(portable_data[val_id]).unwrap_or_default();
                val_id += 1;
            }
            let bytes =
                unsafe { std::slice::from_raw_parts(out_vals.as_ptr() as *const u8, entry_size) };
            if let Some(buf) = self.base.attribute().buffer() {
                buf.borrow_mut().write(out_byte_pos, &bytes);
            }
            out_byte_pos += entry_size as i64;
        }
    }
}

impl Default for SequentialIntegerAttributeDecoder {
    fn default() -> Self {
        Self::new()
    }
}

impl SequentialAttributeDecoderInterface for SequentialIntegerAttributeDecoder {
    fn base(&self) -> &SequentialAttributeDecoderBase {
        &self.base
    }

    fn base_mut(&mut self) -> &mut SequentialAttributeDecoderBase {
        &mut self.base
    }

    fn init(&mut self, decoder: &mut dyn PointCloudDecoder, attribute_id: i32) -> bool {
        self.base.init(decoder, attribute_id)
    }

    fn decode_values(&mut self, point_ids: &[PointIndex], in_buffer: &mut DecoderBuffer) -> bool {
        let mut prediction_scheme_method: i8 = 0;
        if !in_buffer.decode(&mut prediction_scheme_method) {
            return false;
        }
        if prediction_scheme_method < PredictionSchemeMethod::PredictionNone as i8
            || prediction_scheme_method >= PredictionSchemeMethod::NumPredictionSchemes as i8
        {
            return false;
        }
        if prediction_scheme_method != PredictionSchemeMethod::PredictionNone as i8 {
            let mut prediction_transform_type: i8 = 0;
            if !in_buffer.decode(&mut prediction_transform_type) {
                return false;
            }
            if prediction_transform_type
                < PredictionSchemeTransformType::PredictionTransformNone as i8
                || prediction_transform_type
                    >= PredictionSchemeTransformType::NumPredictionSchemeTransformTypes as i8
            {
                return false;
            }
            let method = match Self::prediction_method_from_i8(prediction_scheme_method) {
                Some(m) => m,
                None => return false,
            };
            let transform = match Self::transform_type_from_i8(prediction_transform_type) {
                Some(t) => t,
                None => return false,
            };
            self.prediction_scheme = self.create_int_prediction_scheme(method, transform);
        }

        if let Some(scheme) = &mut self.prediction_scheme {
            if !self.base.init_prediction_scheme(scheme.as_mut()) {
                return false;
            }
        }

        if !self.decode_integer_values(point_ids, in_buffer) {
            return false;
        }

        if let Some(decoder) = self.base.decoder() {
            if decoder.bitstream_version() < bitstream_version(2, 0) {
                if !self.store_values(point_ids.len()) {
                    return false;
                }
            }
        }

        true
    }

    fn transform_attribute_to_original_format(&mut self, point_ids: &[PointIndex]) -> bool {
        if let Some(decoder) = self.base.decoder() {
            if decoder.bitstream_version() < bitstream_version(2, 0) {
                return true;
            }
        }
        self.store_values(point_ids.len())
    }
}
