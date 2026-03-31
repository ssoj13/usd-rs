//! Multi-parallelogram mesh prediction scheme decoder (deprecated).
//! Reference: `_ref/draco/src/draco/compression/attributes/prediction_schemes/mesh_prediction_scheme_multi_parallelogram_decoder.h`.

use num_traits::NumCast;

use crate::compression::attributes::prediction_schemes::mesh_prediction_scheme_corner_table::
    MeshPredictionCornerTable;
use crate::compression::attributes::prediction_schemes::mesh_prediction_scheme_data::
    MeshPredictionSchemeDataRef;
use crate::compression::attributes::prediction_schemes::mesh_prediction_scheme_decoder::
    MeshPredictionSchemeDecoder;
use crate::compression::attributes::prediction_schemes::mesh_prediction_scheme_parallelogram_shared::
    compute_parallelogram_prediction;
use crate::compression::attributes::prediction_schemes::prediction_scheme_decoder_interface::{
    PredictionSchemeDecoderInterface, PredictionSchemeTypedDecoderInterface,
};
use crate::compression::attributes::prediction_schemes::prediction_scheme_decoding_transform::
    DecodingTransform;
use crate::compression::attributes::prediction_schemes::prediction_scheme_interface::
    PredictionSchemeInterface;
use crate::compression::config::compression_shared::PredictionSchemeMethod;
use draco_core::attributes::geometry_attribute::GeometryAttributeType;
use draco_core::attributes::geometry_indices::PointIndex;
use draco_core::attributes::point_attribute::PointAttribute;
use draco_core::core::decoder_buffer::DecoderBuffer;
use draco_core::core::math_utils::{add_as_unsigned, AddAsUnsigned};

pub struct MeshPredictionSchemeMultiParallelogramDecoder<DataTypeT, TransformT, MeshDataT>
where
    TransformT: DecodingTransform<DataTypeT>,
{
    base: MeshPredictionSchemeDecoder<DataTypeT, TransformT, MeshDataT>,
}

impl<DataTypeT, TransformT, MeshDataT>
    MeshPredictionSchemeMultiParallelogramDecoder<DataTypeT, TransformT, MeshDataT>
where
    TransformT: DecodingTransform<DataTypeT>,
{
    pub fn new(attribute: &PointAttribute, transform: TransformT, mesh_data: MeshDataT) -> Self {
        Self {
            base: MeshPredictionSchemeDecoder::new(attribute, transform, mesh_data),
        }
    }
}

impl<DataTypeT, TransformT, MeshDataT> PredictionSchemeInterface
    for MeshPredictionSchemeMultiParallelogramDecoder<DataTypeT, TransformT, MeshDataT>
where
    TransformT: DecodingTransform<DataTypeT>,
    MeshDataT: MeshPredictionSchemeDataRef,
{
    fn get_prediction_method(&self) -> PredictionSchemeMethod {
        PredictionSchemeMethod::MeshPredictionMultiParallelogram
    }

    fn get_attribute(&self) -> &PointAttribute {
        self.base.base().attribute()
    }

    fn is_initialized(&self) -> bool {
        self.base.mesh_data().is_initialized()
    }

    fn get_num_parent_attributes(&self) -> i32 {
        0
    }

    fn get_parent_attribute_type(&self, _i: i32) -> GeometryAttributeType {
        GeometryAttributeType::Invalid
    }

    fn set_parent_attribute(&mut self, _att: &PointAttribute) -> bool {
        false
    }

    fn are_corrections_positive(&self) -> bool {
        self.base.base().transform().are_corrections_positive()
    }

    fn get_transform_type(
        &self,
    ) -> crate::compression::config::compression_shared::PredictionSchemeTransformType {
        self.base.base().transform().get_type()
    }
}

impl<DataTypeT, TransformT, MeshDataT> PredictionSchemeDecoderInterface
    for MeshPredictionSchemeMultiParallelogramDecoder<DataTypeT, TransformT, MeshDataT>
where
    TransformT: DecodingTransform<DataTypeT>,
    MeshDataT: MeshPredictionSchemeDataRef,
{
    fn decode_prediction_data(&mut self, buffer: &mut DecoderBuffer) -> bool {
        self.base
            .base_mut()
            .transform_mut()
            .decode_transform_data(buffer)
    }
}

impl<DataTypeT, TransformT, MeshDataT>
    PredictionSchemeTypedDecoderInterface<DataTypeT, TransformT::CorrType>
    for MeshPredictionSchemeMultiParallelogramDecoder<DataTypeT, TransformT, MeshDataT>
where
    DataTypeT: Copy + Default + NumCast + AddAsUnsigned + std::ops::Div<Output = DataTypeT>,
    TransformT: DecodingTransform<DataTypeT> + Clone,
    TransformT::CorrType: Copy,
    MeshDataT: MeshPredictionSchemeDataRef,
    MeshDataT::CornerTable: MeshPredictionCornerTable,
{
    fn compute_original_values(
        &self,
        in_corr: &[TransformT::CorrType],
        out_data: &mut [DataTypeT],
        num_components: i32,
        _entry_to_point_id_map: &[PointIndex],
    ) -> bool {
        let mut transform = self.base.base().transform().clone();
        transform.init(num_components);

        let num_components_usize = num_components as usize;
        let mut pred_vals = vec![DataTypeT::default(); num_components_usize];
        let mut parallelogram_pred_vals = vec![DataTypeT::default(); num_components_usize];

        if in_corr.is_empty() {
            return true;
        }

        transform.compute_original_value(
            &pred_vals,
            &in_corr[0..num_components_usize],
            &mut out_data[0..num_components_usize],
        );

        let corner_map = self.base.mesh_data().data_to_corner_map();
        let table = self.base.mesh_data().corner_table();
        let vertex_to_data_map = self.base.mesh_data().vertex_to_data_map();

        for p in 1..corner_map.len() {
            let start_corner_id = corner_map[p];
            let mut corner_id = start_corner_id;
            let mut num_parallelograms = 0i32;
            for c in 0..num_components_usize {
                pred_vals[c] = DataTypeT::default();
            }
            let dst_offset = p * num_components_usize;
            let (decoded_prefix, decoded_tail) = out_data.split_at_mut(dst_offset);
            while corner_id != draco_core::attributes::geometry_indices::INVALID_CORNER_INDEX {
                if compute_parallelogram_prediction(
                    p as i32,
                    corner_id,
                    table,
                    vertex_to_data_map,
                    decoded_prefix,
                    num_components,
                    &mut parallelogram_pred_vals,
                ) {
                    for c in 0..num_components_usize {
                        pred_vals[c] = add_as_unsigned(pred_vals[c], parallelogram_pred_vals[c]);
                    }
                    num_parallelograms += 1;
                }
                corner_id = table.swing_right(corner_id);
                if corner_id == start_corner_id {
                    corner_id = draco_core::attributes::geometry_indices::INVALID_CORNER_INDEX;
                }
            }

            let dst_slice = &mut decoded_tail[..num_components_usize];
            if num_parallelograms == 0 {
                let src_offset = (p - 1) * num_components_usize;
                transform.compute_original_value(
                    &decoded_prefix[src_offset..src_offset + num_components_usize],
                    &in_corr[dst_offset..dst_offset + num_components_usize],
                    dst_slice,
                );
            } else {
                let denom: DataTypeT = NumCast::from(num_parallelograms).unwrap_or_default();
                for c in 0..num_components_usize {
                    pred_vals[c] = pred_vals[c] / denom;
                }
                transform.compute_original_value(
                    &pred_vals,
                    &in_corr[dst_offset..dst_offset + num_components_usize],
                    dst_slice,
                );
            }
        }
        true
    }
}
