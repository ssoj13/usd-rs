//! Canonicalized octahedron normal prediction encoding transform (i32).
//! Reference: `_ref/draco/src/draco/compression/attributes/prediction_schemes/prediction_scheme_normal_octahedron_canonicalized_encoding_transform.h`.

use crate::compression::attributes::prediction_schemes::prediction_scheme_encoding_transform::EncodingTransform;
use crate::compression::attributes::prediction_schemes::prediction_scheme_normal_octahedron_canonicalized_transform_base::PredictionSchemeNormalOctahedronCanonicalizedTransformBase;
use crate::compression::config::compression_shared::PredictionSchemeTransformType;
use draco_core::core::encoder_buffer::EncoderBuffer;

#[derive(Clone)]
pub struct PredictionSchemeNormalOctahedronCanonicalizedEncodingTransform {
    base: PredictionSchemeNormalOctahedronCanonicalizedTransformBase,
}

impl PredictionSchemeNormalOctahedronCanonicalizedEncodingTransform {
    pub fn new(max_quantized_value: i32) -> Self {
        Self {
            base:
                PredictionSchemeNormalOctahedronCanonicalizedTransformBase::with_max_quantized_value(
                    max_quantized_value,
                ),
        }
    }

    pub fn with_max_quantized_value(max_quantized_value: i32) -> Self {
        Self::new(max_quantized_value)
    }

    pub fn max_quantized_value(&self) -> i32 {
        self.base.base().max_quantized_value()
    }
    pub fn center_value(&self) -> i32 {
        self.base.base().center_value()
    }
    pub fn quantization_bits(&self) -> i32 {
        self.base.base().quantization_bits()
    }

    fn compute_correction_internal(&self, orig: [i32; 2], pred: [i32; 2]) -> [i32; 2] {
        let t = self.base.base().center_value();
        let mut orig = [orig[0] - t, orig[1] - t];
        let mut pred = [pred[0] - t, pred[1] - t];
        if !self.base.base().is_in_diamond(pred[0], pred[1]) {
            let mut s = orig[0];
            let mut u = orig[1];
            self.base.base().invert_diamond(&mut s, &mut u);
            orig = [s, u];
            let mut s = pred[0];
            let mut u = pred[1];
            self.base.base().invert_diamond(&mut s, &mut u);
            pred = [s, u];
        }
        if !self.base.is_in_bottom_left(pred) {
            let rotation_count = self.base.get_rotation_count(pred);
            orig = self.base.rotate_point(orig, rotation_count);
            pred = self.base.rotate_point(pred, rotation_count);
        }
        let mut corr = [orig[0] - pred[0], orig[1] - pred[1]];
        corr[0] = self.base.base().make_positive(corr[0]);
        corr[1] = self.base.base().make_positive(corr[1]);
        corr
    }
}

impl EncodingTransform<i32> for PredictionSchemeNormalOctahedronCanonicalizedEncodingTransform {
    type CorrType = i32;

    fn get_type(&self) -> PredictionSchemeTransformType {
        PredictionSchemeNormalOctahedronCanonicalizedTransformBase::get_type()
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
        buffer.encode(self.base.base().max_quantized_value())
            && buffer.encode(self.base.base().center_value())
    }

    fn are_corrections_positive(&self) -> bool {
        true
    }
}
