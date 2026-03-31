//! Canonicalized octahedron normal prediction decoding transform (i32).
//! Reference: `_ref/draco/src/draco/compression/attributes/prediction_schemes/prediction_scheme_normal_octahedron_canonicalized_decoding_transform.h`.

use crate::compression::attributes::prediction_schemes::prediction_scheme_decoding_transform::DecodingTransform;
use crate::compression::attributes::prediction_schemes::prediction_scheme_normal_octahedron_canonicalized_transform_base::PredictionSchemeNormalOctahedronCanonicalizedTransformBase;
use crate::compression::config::compression_shared::PredictionSchemeTransformType;
use draco_core::core::decoder_buffer::DecoderBuffer;
use draco_core::core::math_utils::add_as_unsigned;

#[derive(Clone)]
pub struct PredictionSchemeNormalOctahedronCanonicalizedDecodingTransform {
    base: PredictionSchemeNormalOctahedronCanonicalizedTransformBase,
}

impl PredictionSchemeNormalOctahedronCanonicalizedDecodingTransform {
    pub fn new() -> Self {
        Self {
            base: PredictionSchemeNormalOctahedronCanonicalizedTransformBase::new(),
        }
    }
}

impl DecodingTransform<i32> for PredictionSchemeNormalOctahedronCanonicalizedDecodingTransform {
    type CorrType = i32;

    fn get_type(&self) -> PredictionSchemeTransformType {
        PredictionSchemeNormalOctahedronCanonicalizedTransformBase::get_type()
    }

    fn init(&mut self, _num_components: i32) {}

    fn compute_original_value(
        &self,
        predicted_vals: &[i32],
        corr_vals: &[Self::CorrType],
        out_original_vals: &mut [i32],
    ) {
        let t = self.base.base().center_value();
        let mut pred = [predicted_vals[0], predicted_vals[1]];
        pred = [pred[0] - t, pred[1] - t];
        let pred_is_in_diamond = self.base.base().is_in_diamond(pred[0], pred[1]);
        if !pred_is_in_diamond {
            let mut s = pred[0];
            let mut u = pred[1];
            self.base.base().invert_diamond(&mut s, &mut u);
            pred = [s, u];
        }
        let pred_is_in_bottom_left = self.base.is_in_bottom_left(pred);
        let rotation_count = self.base.get_rotation_count(pred);
        if !pred_is_in_bottom_left {
            pred = self.base.rotate_point(pred, rotation_count);
        }
        let mut orig = [
            self.base
                .base()
                .mod_max(add_as_unsigned(pred[0], corr_vals[0])),
            self.base
                .base()
                .mod_max(add_as_unsigned(pred[1], corr_vals[1])),
        ];
        if !pred_is_in_bottom_left {
            let reverse_rotation_count = (4 - rotation_count) % 4;
            orig = self.base.rotate_point(orig, reverse_rotation_count);
        }
        if !pred_is_in_diamond {
            let mut s = orig[0];
            let mut u = orig[1];
            self.base.base().invert_diamond(&mut s, &mut u);
            orig = [s, u];
        }
        orig = [orig[0] + t, orig[1] + t];
        out_original_vals[0] = orig[0];
        out_original_vals[1] = orig[1];
    }

    fn decode_transform_data(&mut self, buffer: &mut DecoderBuffer) -> bool {
        let mut max_quantized_value: i32 = 0;
        let mut center_value: i32 = 0;
        if !buffer.decode(&mut max_quantized_value) {
            return false;
        }
        if !buffer.decode(&mut center_value) {
            return false;
        }
        let _ = center_value;
        if !self
            .base
            .base_mut()
            .set_max_quantized_value(max_quantized_value)
        {
            return false;
        }
        let q = self.base.base().quantization_bits();
        if q < 2 || q > 30 {
            return false;
        }
        true
    }

    fn are_corrections_positive(&self) -> bool {
        true
    }

    fn quantization_bits(&self) -> i32 {
        self.base.base().quantization_bits()
    }
}

impl Default for PredictionSchemeNormalOctahedronCanonicalizedDecodingTransform {
    fn default() -> Self {
        Self::new()
    }
}
