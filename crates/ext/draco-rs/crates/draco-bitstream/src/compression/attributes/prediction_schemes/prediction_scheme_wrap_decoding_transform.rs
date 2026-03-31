//! Wrap prediction decoding transform (i32).
//! Reference: `_ref/draco/src/draco/compression/attributes/prediction_schemes/prediction_scheme_wrap_decoding_transform.h`.

use crate::compression::attributes::prediction_schemes::prediction_scheme_decoding_transform::DecodingTransform;
use crate::compression::attributes::prediction_schemes::prediction_scheme_wrap_transform_base::PredictionSchemeWrapTransformBase;
use crate::compression::config::compression_shared::PredictionSchemeTransformType;
use draco_core::core::decoder_buffer::DecoderBuffer;

#[derive(Clone)]
pub struct PredictionSchemeWrapDecodingTransform {
    base: PredictionSchemeWrapTransformBase,
}

impl PredictionSchemeWrapDecodingTransform {
    pub fn new() -> Self {
        Self {
            base: PredictionSchemeWrapTransformBase::new(),
        }
    }
}

impl DecodingTransform<i32> for PredictionSchemeWrapDecodingTransform {
    type CorrType = i32;

    fn get_type(&self) -> PredictionSchemeTransformType {
        PredictionSchemeWrapTransformBase::get_type()
    }

    fn init(&mut self, num_components: i32) {
        self.base.init(num_components);
    }

    fn compute_original_value(
        &self,
        predicted_vals: &[i32],
        corr_vals: &[Self::CorrType],
        out_original_vals: &mut [i32],
    ) {
        let clamped = self.base.clamp_predicted_value(predicted_vals);
        for i in 0..self.base.num_components() as usize {
            let unsigned_pred = clamped[i] as u32;
            let unsigned_corr = corr_vals[i] as u32;
            let mut value = unsigned_pred.wrapping_add(unsigned_corr) as i32;
            if value > self.base.max_value() {
                value -= self.base.max_dif();
            } else if value < self.base.min_value() {
                value += self.base.max_dif();
            }
            out_original_vals[i] = value;
        }
    }

    fn decode_transform_data(&mut self, buffer: &mut DecoderBuffer) -> bool {
        let mut min_value: i32 = 0;
        let mut max_value: i32 = 0;
        if !buffer.decode(&mut min_value) {
            return false;
        }
        if !buffer.decode(&mut max_value) {
            return false;
        }
        if min_value > max_value {
            return false;
        }
        self.base.set_min_value(min_value);
        self.base.set_max_value(max_value);
        self.base.init_correction_bounds()
    }

    fn are_corrections_positive(&self) -> bool {
        self.base.are_corrections_positive()
    }
}

impl Default for PredictionSchemeWrapDecodingTransform {
    fn default() -> Self {
        Self::new()
    }
}
