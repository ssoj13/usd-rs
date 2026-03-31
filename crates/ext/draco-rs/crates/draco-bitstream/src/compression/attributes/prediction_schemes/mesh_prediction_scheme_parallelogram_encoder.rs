//! Parallelogram mesh prediction scheme encoder.
//! Reference: `_ref/draco/src/draco/compression/attributes/prediction_schemes/mesh_prediction_scheme_parallelogram_encoder.h`.

use num_traits::NumCast;

use crate::compression::attributes::prediction_schemes::mesh_prediction_scheme_corner_table::MeshPredictionCornerTable;
use crate::compression::attributes::prediction_schemes::mesh_prediction_scheme_data::MeshPredictionSchemeDataRef;
use crate::compression::attributes::prediction_schemes::mesh_prediction_scheme_parallelogram_shared::compute_parallelogram_prediction;
use crate::compression::attributes::prediction_schemes::prediction_scheme_encoder::PredictionSchemeEncoder;
use crate::compression::attributes::prediction_schemes::prediction_scheme_encoder_interface::{
    PredictionSchemeEncoderInterface, PredictionSchemeTypedEncoderInterface,
};
use crate::compression::attributes::prediction_schemes::prediction_scheme_encoding_transform::EncodingTransform;
use crate::compression::attributes::prediction_schemes::prediction_scheme_interface::PredictionSchemeInterface;
use crate::compression::config::compression_shared::{
    PredictionSchemeMethod, PredictionSchemeTransformType,
};
use draco_core::attributes::geometry_attribute::GeometryAttributeType;
use draco_core::attributes::geometry_indices::PointIndex;
use draco_core::attributes::point_attribute::PointAttribute;
use draco_core::core::encoder_buffer::EncoderBuffer;

pub struct MeshPredictionSchemeParallelogramEncoder<DataTypeT, TransformT, MeshDataT>
where
    TransformT: EncodingTransform<DataTypeT>,
{
    base: PredictionSchemeEncoder<DataTypeT, TransformT>,
    mesh_data: MeshDataT,
}

impl<DataTypeT, TransformT, MeshDataT>
    MeshPredictionSchemeParallelogramEncoder<DataTypeT, TransformT, MeshDataT>
where
    TransformT: EncodingTransform<DataTypeT>,
{
    pub fn new(attribute: &PointAttribute, transform: TransformT, mesh_data: MeshDataT) -> Self {
        Self {
            base: PredictionSchemeEncoder::new(attribute, transform),
            mesh_data,
        }
    }

    pub fn base(&self) -> &PredictionSchemeEncoder<DataTypeT, TransformT> {
        &self.base
    }

    pub fn base_mut(&mut self) -> &mut PredictionSchemeEncoder<DataTypeT, TransformT> {
        &mut self.base
    }

    pub fn mesh_data(&self) -> &MeshDataT {
        &self.mesh_data
    }
}

impl<DataTypeT, TransformT, MeshDataT> PredictionSchemeInterface
    for MeshPredictionSchemeParallelogramEncoder<DataTypeT, TransformT, MeshDataT>
where
    TransformT: EncodingTransform<DataTypeT>,
    MeshDataT: MeshPredictionSchemeDataRef,
{
    fn get_prediction_method(&self) -> PredictionSchemeMethod {
        PredictionSchemeMethod::MeshPredictionParallelogram
    }

    fn get_attribute(&self) -> &PointAttribute {
        self.base.attribute()
    }

    fn is_initialized(&self) -> bool {
        self.mesh_data.is_initialized()
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
        self.base.transform().are_corrections_positive()
    }

    fn get_transform_type(&self) -> PredictionSchemeTransformType {
        self.base.transform().get_type()
    }
}

impl<DataTypeT, TransformT, MeshDataT> PredictionSchemeEncoderInterface
    for MeshPredictionSchemeParallelogramEncoder<DataTypeT, TransformT, MeshDataT>
where
    TransformT: EncodingTransform<DataTypeT>,
    MeshDataT: MeshPredictionSchemeDataRef,
{
    fn encode_prediction_data(&self, buffer: &mut EncoderBuffer) -> bool {
        self.base.transform().encode_transform_data(buffer)
    }
}

impl<DataTypeT, TransformT, MeshDataT>
    PredictionSchemeTypedEncoderInterface<DataTypeT, TransformT::CorrType>
    for MeshPredictionSchemeParallelogramEncoder<DataTypeT, TransformT, MeshDataT>
where
    DataTypeT: Copy + Default + NumCast,
    TransformT: EncodingTransform<DataTypeT> + Clone,
    TransformT::CorrType: Copy,
    MeshDataT: MeshPredictionSchemeDataRef,
    MeshDataT::CornerTable: MeshPredictionCornerTable,
{
    fn compute_correction_values(
        &mut self,
        in_data: &[DataTypeT],
        out_corr: &mut [TransformT::CorrType],
        num_components: i32,
        _entry_to_point_id_map: &[PointIndex],
    ) -> bool {
        let num_components_usize = num_components as usize;
        self.base_mut()
            .transform_mut()
            .init(in_data, in_data.len() as i32, num_components);
        let transform = self.base.transform();

        let mut pred_vals = vec![DataTypeT::default(); num_components_usize];

        let corner_map = self.mesh_data.data_to_corner_map();
        let table = self.mesh_data.corner_table();
        let vertex_to_data_map = self.mesh_data.vertex_to_data_map();

        let num_entries = corner_map.len();
        if num_entries <= 1 {
            if num_entries == 1 {
                transform.compute_correction(
                    &in_data[0..num_components_usize],
                    &pred_vals,
                    &mut out_corr[0..num_components_usize],
                );
            }
            return true;
        }

        for p in (1..num_entries).rev() {
            let corner_id = corner_map[p];
            let dst_offset = p * num_components_usize;
            let has_pred = compute_parallelogram_prediction(
                p as i32,
                corner_id,
                table,
                vertex_to_data_map,
                in_data,
                num_components,
                &mut pred_vals,
            );

            if !has_pred {
                let src_offset = (p - 1) * num_components_usize;
                transform.compute_correction(
                    &in_data[dst_offset..dst_offset + num_components_usize],
                    &in_data[src_offset..src_offset + num_components_usize],
                    &mut out_corr[dst_offset..dst_offset + num_components_usize],
                );
            } else {
                transform.compute_correction(
                    &in_data[dst_offset..dst_offset + num_components_usize],
                    &pred_vals,
                    &mut out_corr[dst_offset..dst_offset + num_components_usize],
                );
            }
        }

        for v in pred_vals.iter_mut() {
            *v = DataTypeT::default();
        }
        transform.compute_correction(
            &in_data[0..num_components_usize],
            &pred_vals,
            &mut out_corr[0..num_components_usize],
        );
        true
    }
}
