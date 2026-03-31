//! Prediction scheme decoding transform (delta by default).
//! Reference: `_ref/draco/src/draco/compression/attributes/prediction_schemes/prediction_scheme_decoding_transform.h`.

use crate::compression::config::compression_shared::PredictionSchemeTransformType;
use draco_core::core::decoder_buffer::DecoderBuffer;

pub trait DecodingTransform<DataTypeT> {
    type CorrType;
    fn get_type(&self) -> PredictionSchemeTransformType;
    fn init(&mut self, num_components: i32);
    fn compute_original_value(
        &self,
        predicted_vals: &[DataTypeT],
        corr_vals: &[Self::CorrType],
        out_original_vals: &mut [DataTypeT],
    );
    fn decode_transform_data(&mut self, buffer: &mut DecoderBuffer) -> bool;
    fn are_corrections_positive(&self) -> bool;
    fn quantization_bits(&self) -> i32 {
        0
    }
}

#[derive(Clone)]
pub struct PredictionSchemeDecodingTransform<DataTypeT, CorrTypeT> {
    num_components: i32,
    _phantom: std::marker::PhantomData<(DataTypeT, CorrTypeT)>,
}

impl<DataTypeT, CorrTypeT> PredictionSchemeDecodingTransform<DataTypeT, CorrTypeT>
where
    DataTypeT: Copy + std::ops::Add<Output = DataTypeT>,
    CorrTypeT: Copy + Into<DataTypeT>,
{
    pub fn new() -> Self {
        Self {
            num_components: 0,
            _phantom: std::marker::PhantomData,
        }
    }

    pub fn num_components(&self) -> i32 {
        self.num_components
    }
}

impl<DataTypeT, CorrTypeT> DecodingTransform<DataTypeT>
    for PredictionSchemeDecodingTransform<DataTypeT, CorrTypeT>
where
    DataTypeT: Copy + std::ops::Add<Output = DataTypeT>,
    CorrTypeT: Copy + Into<DataTypeT>,
{
    type CorrType = CorrTypeT;

    fn get_type(&self) -> PredictionSchemeTransformType {
        PredictionSchemeTransformType::PredictionTransformDelta
    }

    fn init(&mut self, num_components: i32) {
        self.num_components = num_components;
    }

    fn compute_original_value(
        &self,
        predicted_vals: &[DataTypeT],
        corr_vals: &[Self::CorrType],
        out_original_vals: &mut [DataTypeT],
    ) {
        for i in 0..self.num_components as usize {
            out_original_vals[i] = predicted_vals[i] + corr_vals[i].into();
        }
    }

    fn decode_transform_data(&mut self, _buffer: &mut DecoderBuffer) -> bool {
        true
    }

    fn are_corrections_positive(&self) -> bool {
        false
    }
}

impl<DataTypeT, CorrTypeT> Default for PredictionSchemeDecodingTransform<DataTypeT, CorrTypeT>
where
    DataTypeT: Copy + std::ops::Add<Output = DataTypeT>,
    CorrTypeT: Copy + Into<DataTypeT>,
{
    fn default() -> Self {
        Self::new()
    }
}
