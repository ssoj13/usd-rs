//! Prediction scheme decoder interfaces.
//! Reference: `_ref/draco/src/draco/compression/attributes/prediction_schemes/prediction_scheme_decoder_interface.h`.

use crate::compression::attributes::prediction_schemes::prediction_scheme_interface::PredictionSchemeInterface;
use draco_core::attributes::geometry_indices::PointIndex;
use draco_core::core::decoder_buffer::DecoderBuffer;

pub trait PredictionSchemeDecoderInterface: PredictionSchemeInterface {
    fn decode_prediction_data(&mut self, buffer: &mut DecoderBuffer) -> bool;
}

pub trait PredictionSchemeTypedDecoderInterface<DataTypeT, CorrTypeT>:
    PredictionSchemeDecoderInterface
{
    fn compute_original_values(
        &self,
        in_corr: &[CorrTypeT],
        out_data: &mut [DataTypeT],
        num_components: i32,
        entry_to_point_id_map: &[PointIndex],
    ) -> bool;
}
