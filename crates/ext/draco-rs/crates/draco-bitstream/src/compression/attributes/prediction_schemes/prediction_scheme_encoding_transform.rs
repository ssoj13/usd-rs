//! Prediction scheme encoding transform (delta by default).
//! Reference: `_ref/draco/src/draco/compression/attributes/prediction_schemes/prediction_scheme_encoding_transform.h`.

use crate::compression::config::compression_shared::PredictionSchemeTransformType;
use draco_core::core::encoder_buffer::EncoderBuffer;

pub trait EncodingTransform<DataTypeT> {
    type CorrType;
    fn get_type(&self) -> PredictionSchemeTransformType;
    fn init(&mut self, orig_data: &[DataTypeT], size: i32, num_components: i32);
    fn compute_correction(
        &self,
        original_vals: &[DataTypeT],
        predicted_vals: &[DataTypeT],
        out_corr_vals: &mut [Self::CorrType],
    );
    fn encode_transform_data(&self, buffer: &mut EncoderBuffer) -> bool;
    fn are_corrections_positive(&self) -> bool;
}

#[derive(Clone)]
pub struct PredictionSchemeEncodingTransform<DataTypeT, CorrTypeT> {
    num_components: i32,
    _phantom: std::marker::PhantomData<(DataTypeT, CorrTypeT)>,
}

impl<DataTypeT, CorrTypeT> PredictionSchemeEncodingTransform<DataTypeT, CorrTypeT>
where
    DataTypeT: Copy + std::ops::Sub<Output = DataTypeT>,
    CorrTypeT: From<DataTypeT> + Copy,
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

impl<DataTypeT, CorrTypeT> EncodingTransform<DataTypeT>
    for PredictionSchemeEncodingTransform<DataTypeT, CorrTypeT>
where
    DataTypeT: Copy + std::ops::Sub<Output = DataTypeT>,
    CorrTypeT: From<DataTypeT> + Copy,
{
    type CorrType = CorrTypeT;

    fn get_type(&self) -> PredictionSchemeTransformType {
        PredictionSchemeTransformType::PredictionTransformDelta
    }

    fn init(&mut self, _orig_data: &[DataTypeT], _size: i32, num_components: i32) {
        self.num_components = num_components;
    }

    fn compute_correction(
        &self,
        original_vals: &[DataTypeT],
        predicted_vals: &[DataTypeT],
        out_corr_vals: &mut [Self::CorrType],
    ) {
        for i in 0..self.num_components as usize {
            out_corr_vals[i] = CorrTypeT::from(original_vals[i] - predicted_vals[i]);
        }
    }

    fn encode_transform_data(&self, _buffer: &mut EncoderBuffer) -> bool {
        true
    }

    fn are_corrections_positive(&self) -> bool {
        false
    }
}

impl<DataTypeT, CorrTypeT> Default for PredictionSchemeEncodingTransform<DataTypeT, CorrTypeT>
where
    DataTypeT: Copy + std::ops::Sub<Output = DataTypeT>,
    CorrTypeT: From<DataTypeT> + Copy,
{
    fn default() -> Self {
        Self::new()
    }
}
