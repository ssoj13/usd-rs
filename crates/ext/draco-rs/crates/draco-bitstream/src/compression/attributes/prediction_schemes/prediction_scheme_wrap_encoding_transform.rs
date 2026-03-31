//! Wrap prediction encoding transform (i32).
//! Reference: `_ref/draco/src/draco/compression/attributes/prediction_schemes/prediction_scheme_wrap_encoding_transform.h`.

use crate::compression::attributes::prediction_schemes::prediction_scheme_encoding_transform::EncodingTransform;
use crate::compression::attributes::prediction_schemes::prediction_scheme_wrap_transform_base::PredictionSchemeWrapTransformBase;
use crate::compression::config::compression_shared::PredictionSchemeTransformType;
use draco_core::core::encoder_buffer::EncoderBuffer;

#[derive(Clone)]
pub struct PredictionSchemeWrapEncodingTransform {
    base: PredictionSchemeWrapTransformBase,
}

impl PredictionSchemeWrapEncodingTransform {
    pub fn new() -> Self {
        Self {
            base: PredictionSchemeWrapTransformBase::new(),
        }
    }
}

impl EncodingTransform<i32> for PredictionSchemeWrapEncodingTransform {
    type CorrType = i32;

    fn get_type(&self) -> PredictionSchemeTransformType {
        PredictionSchemeWrapTransformBase::get_type()
    }

    fn init(&mut self, orig_data: &[i32], _size: i32, num_components: i32) {
        self.base.init(num_components);
        if orig_data.is_empty() {
            return;
        }
        let mut min_value = orig_data[0];
        let mut max_value = orig_data[0];
        for &v in &orig_data[1..] {
            if v < min_value {
                min_value = v;
            } else if v > max_value {
                max_value = v;
            }
        }
        self.base.set_min_value(min_value);
        self.base.set_max_value(max_value);
        let _ = self.base.init_correction_bounds();
    }

    fn compute_correction(
        &self,
        original_vals: &[i32],
        predicted_vals: &[i32],
        out_corr_vals: &mut [Self::CorrType],
    ) {
        let clamped = self.base.clamp_predicted_value(predicted_vals);
        for i in 0..self.base.num_components() as usize {
            let mut corr_val = original_vals[i] - clamped[i];
            if corr_val < self.base.min_correction() {
                corr_val += self.base.max_dif();
            } else if corr_val > self.base.max_correction() {
                corr_val -= self.base.max_dif();
            }
            out_corr_vals[i] = corr_val;
        }
    }

    fn encode_transform_data(&self, buffer: &mut EncoderBuffer) -> bool {
        buffer.encode(self.base.min_value()) && buffer.encode(self.base.max_value())
    }

    fn are_corrections_positive(&self) -> bool {
        self.base.are_corrections_positive()
    }
}

impl Default for PredictionSchemeWrapEncodingTransform {
    fn default() -> Self {
        Self::new()
    }
}
