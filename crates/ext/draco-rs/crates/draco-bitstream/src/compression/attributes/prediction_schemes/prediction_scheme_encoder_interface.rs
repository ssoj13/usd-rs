//! Prediction scheme encoder interfaces.
//! Reference: `_ref/draco/src/draco/compression/attributes/prediction_schemes/prediction_scheme_encoder_interface.h`.

use crate::compression::attributes::prediction_schemes::prediction_scheme_interface::PredictionSchemeInterface;
use draco_core::attributes::geometry_indices::PointIndex;
use draco_core::core::encoder_buffer::EncoderBuffer;

pub trait PredictionSchemeEncoderInterface: PredictionSchemeInterface {
    fn encode_prediction_data(&self, buffer: &mut EncoderBuffer) -> bool;
}

pub trait PredictionSchemeTypedEncoderInterface<DataTypeT, CorrTypeT>:
    PredictionSchemeEncoderInterface
{
    fn compute_correction_values(
        &mut self,
        in_data: &[DataTypeT],
        out_corr: &mut [CorrTypeT],
        num_components: i32,
        entry_to_point_id_map: &[PointIndex],
    ) -> bool;
}
