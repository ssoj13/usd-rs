//! Octahedron normal prediction encoding transform (i32).
//! Reference: `_ref/draco/src/draco/compression/attributes/prediction_schemes/prediction_scheme_normal_octahedron_encoding_transform.h`.

use crate::compression::attributes::prediction_schemes::prediction_scheme_encoding_transform::EncodingTransform;
use crate::compression::attributes::prediction_schemes::prediction_scheme_normal_octahedron_transform_base::PredictionSchemeNormalOctahedronTransformBase;
use crate::compression::config::compression_shared::PredictionSchemeTransformType;
use draco_core::core::encoder_buffer::EncoderBuffer;

#[derive(Clone)]
pub struct PredictionSchemeNormalOctahedronEncodingTransform {
    base: PredictionSchemeNormalOctahedronTransformBase,
}

impl PredictionSchemeNormalOctahedronEncodingTransform {
    pub fn new(max_quantized_value: i32) -> Self {
        Self {
            base: PredictionSchemeNormalOctahedronTransformBase::with_max_quantized_value(
                max_quantized_value,
            ),
        }
    }

    pub fn with_max_quantized_value(max_quantized_value: i32) -> Self {
        Self::new(max_quantized_value)
    }

    pub fn max_quantized_value(&self) -> i32 {
        self.base.max_quantized_value()
    }
    pub fn center_value(&self) -> i32 {
        self.base.center_value()
    }
    pub fn quantization_bits(&self) -> i32 {
        self.base.quantization_bits()
    }

    fn compute_correction_internal(&self, orig: [i32; 2], pred: [i32; 2]) -> [i32; 2] {
        let t = self.base.center_value();
        let mut orig = [orig[0] - t, orig[1] - t];
        let mut pred = [pred[0] - t, pred[1] - t];
        if !self.base.is_in_diamond(pred[0], pred[1]) {
            let mut s = orig[0];
            let mut u = orig[1];
            self.base.invert_diamond(&mut s, &mut u);
            orig = [s, u];
            let mut s = pred[0];
            let mut u = pred[1];
            self.base.invert_diamond(&mut s, &mut u);
            pred = [s, u];
        }
        let mut corr = [orig[0] - pred[0], orig[1] - pred[1]];
        corr[0] = self.base.make_positive(corr[0]);
        corr[1] = self.base.make_positive(corr[1]);
        corr
    }
}

impl EncodingTransform<i32> for PredictionSchemeNormalOctahedronEncodingTransform {
    type CorrType = i32;

    fn get_type(&self) -> PredictionSchemeTransformType {
        PredictionSchemeNormalOctahedronTransformBase::get_type()
    }

    fn init(&mut self, _orig_data: &[i32], _size: i32, _num_components: i32) {}

    fn compute_correction(
        &self,
        original_vals: &[i32],
        predicted_vals: &[i32],
        out_corr_vals: &mut [Self::CorrType],
    ) {
        let corr = self.compute_correction_internal(
            [original_vals[0], original_vals[1]],
            [predicted_vals[0], predicted_vals[1]],
        );
        out_corr_vals[0] = corr[0];
        out_corr_vals[1] = corr[1];
    }

    fn encode_transform_data(&self, buffer: &mut EncoderBuffer) -> bool {
        buffer.encode(self.base.max_quantized_value())
    }

    fn are_corrections_positive(&self) -> bool {
        true
    }
}
