//! Octahedron normal prediction decoding transform (i32, legacy).
//! Reference: `_ref/draco/src/draco/compression/attributes/prediction_schemes/prediction_scheme_normal_octahedron_decoding_transform.h`.

use crate::compression::attributes::prediction_schemes::prediction_scheme_decoding_transform::DecodingTransform;
use crate::compression::attributes::prediction_schemes::prediction_scheme_normal_octahedron_transform_base::PredictionSchemeNormalOctahedronTransformBase;
use crate::compression::config::compression_shared::PredictionSchemeTransformType;
use draco_core::core::decoder_buffer::DecoderBuffer;

#[derive(Clone)]
pub struct PredictionSchemeNormalOctahedronDecodingTransform {
    base: PredictionSchemeNormalOctahedronTransformBase,
}

impl PredictionSchemeNormalOctahedronDecodingTransform {
    pub fn new() -> Self {
        Self {
            base: PredictionSchemeNormalOctahedronTransformBase::new(),
        }
    }
}

impl DecodingTransform<i32> for PredictionSchemeNormalOctahedronDecodingTransform {
    type CorrType = i32;

    fn get_type(&self) -> PredictionSchemeTransformType {
        PredictionSchemeNormalOctahedronTransformBase::get_type()
    }

    fn init(&mut self, _num_components: i32) {}

    fn compute_original_value(
        &self,
        predicted_vals: &[i32],
        corr_vals: &[Self::CorrType],
        out_original_vals: &mut [i32],
    ) {
        let pred = [predicted_vals[0], predicted_vals[1]];
        let corr = [corr_vals[0], corr_vals[1]];
        let mut pred = pred;
        let t = self.base.center_value();
        pred = [pred[0] - t, pred[1] - t];
        let pred_is_in_diamond = self.base.is_in_diamond(pred[0], pred[1]);
        if !pred_is_in_diamond {
            let mut s = pred[0];
            let mut u = pred[1];
            self.base.invert_diamond(&mut s, &mut u);
            pred = [s, u];
        }
        let mut orig = [
            self.base.mod_max(pred[0] + corr[0]),
            self.base.mod_max(pred[1] + corr[1]),
        ];
        if !pred_is_in_diamond {
            let mut s = orig[0];
            let mut u = orig[1];
            self.base.invert_diamond(&mut s, &mut u);
            orig = [s, u];
        }
        orig = [orig[0] + t, orig[1] + t];
        out_original_vals[0] = orig[0];
        out_original_vals[1] = orig[1];
    }

    fn decode_transform_data(&mut self, buffer: &mut DecoderBuffer) -> bool {
        let mut max_quantized_value: i32 = 0;
        if !buffer.decode(&mut max_quantized_value) {
            return false;
        }
        let mut center_value: i32 = 0;
        if buffer.bitstream_version()
            < crate::compression::config::compression_shared::bitstream_version(2, 2)
        {
            if !buffer.decode(&mut center_value) {
                return false;
            }
        }
        let _ = center_value;
        self.base.set_max_quantized_value(max_quantized_value)
    }

    fn are_corrections_positive(&self) -> bool {
        true
    }

    fn quantization_bits(&self) -> i32 {
        self.base.quantization_bits()
    }
}

impl Default for PredictionSchemeNormalOctahedronDecodingTransform {
    fn default() -> Self {
        Self::new()
    }
}
