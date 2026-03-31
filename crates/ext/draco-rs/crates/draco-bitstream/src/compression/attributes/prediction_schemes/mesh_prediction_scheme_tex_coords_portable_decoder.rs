//! Portable texcoords mesh prediction scheme decoder.
//! Reference: `_ref/draco/src/draco/compression/attributes/prediction_schemes/mesh_prediction_scheme_tex_coords_portable_decoder.h`.

use num_traits::NumCast;

use crate::compression::attributes::prediction_schemes::mesh_prediction_scheme_corner_table::
    MeshPredictionCornerTable;
use crate::compression::attributes::prediction_schemes::mesh_prediction_scheme_data::
    MeshPredictionSchemeDataRef;
use crate::compression::attributes::prediction_schemes::mesh_prediction_scheme_decoder::
    MeshPredictionSchemeDecoder;
use crate::compression::attributes::prediction_schemes::mesh_prediction_scheme_tex_coords_portable_predictor::
    MeshPredictionSchemeTexCoordsPortablePredictor;
use crate::compression::attributes::prediction_schemes::prediction_scheme_decoder_interface::{
    PredictionSchemeDecoderInterface, PredictionSchemeTypedDecoderInterface,
};
use crate::compression::attributes::prediction_schemes::prediction_scheme_decoding_transform::
    DecodingTransform;
use crate::compression::attributes::prediction_schemes::prediction_scheme_interface::
    PredictionSchemeInterface;
use crate::compression::bit_coders::rans_bit_decoder::RAnsBitDecoder;
use crate::compression::config::compression_shared::PredictionSchemeMethod;
use draco_core::attributes::geometry_attribute::GeometryAttributeType;
use draco_core::attributes::geometry_indices::PointIndex;
use draco_core::attributes::point_attribute::PointAttribute;
use draco_core::core::decoder_buffer::DecoderBuffer;

pub struct MeshPredictionSchemeTexCoordsPortableDecoder<DataTypeT, TransformT, MeshDataT>
where
    TransformT: DecodingTransform<DataTypeT>,
    MeshDataT: MeshPredictionSchemeDataRef + Copy,
    MeshDataT::CornerTable: MeshPredictionCornerTable,
{
    base: MeshPredictionSchemeDecoder<DataTypeT, TransformT, MeshDataT>,
    predictor: MeshPredictionSchemeTexCoordsPortablePredictor<DataTypeT, MeshDataT>,
}

impl<DataTypeT, TransformT, MeshDataT>
    MeshPredictionSchemeTexCoordsPortableDecoder<DataTypeT, TransformT, MeshDataT>
where
    DataTypeT: Copy + Default + NumCast,
    TransformT: DecodingTransform<DataTypeT>,
    MeshDataT: MeshPredictionSchemeDataRef + Copy,
    MeshDataT::CornerTable: MeshPredictionCornerTable,
{
    pub fn new(attribute: &PointAttribute, transform: TransformT, mesh_data: MeshDataT) -> Self {
        let predictor = MeshPredictionSchemeTexCoordsPortablePredictor::new(mesh_data);
        Self {
            base: MeshPredictionSchemeDecoder::new(attribute, transform, mesh_data),
            predictor,
        }
    }
}

impl<DataTypeT, TransformT, MeshDataT> PredictionSchemeInterface
    for MeshPredictionSchemeTexCoordsPortableDecoder<DataTypeT, TransformT, MeshDataT>
where
    DataTypeT: Copy + Default + NumCast,
    TransformT: DecodingTransform<DataTypeT>,
    MeshDataT: MeshPredictionSchemeDataRef + Copy,
    MeshDataT::CornerTable: MeshPredictionCornerTable,
{
    fn get_prediction_method(&self) -> PredictionSchemeMethod {
        PredictionSchemeMethod::MeshPredictionTexCoordsPortable
    }

    fn get_attribute(&self) -> &PointAttribute {
        self.base.base().attribute()
    }

    fn is_initialized(&self) -> bool {
        self.predictor.is_initialized() && self.base.mesh_data().is_initialized()
    }

    fn get_num_parent_attributes(&self) -> i32 {
        1
    }

    fn get_parent_attribute_type(&self, i: i32) -> GeometryAttributeType {
        if i == 0 {
            GeometryAttributeType::Position
        } else {
            GeometryAttributeType::Invalid
        }
    }

    fn set_parent_attribute(&mut self, att: &PointAttribute) -> bool {
        if att.attribute_type() != GeometryAttributeType::Position {
            return false;
        }
        if att.num_components() != 3 {
            return false;
        }
        self.predictor.set_position_attribute(att);
        true
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
    for MeshPredictionSchemeTexCoordsPortableDecoder<DataTypeT, TransformT, MeshDataT>
where
    DataTypeT: Copy + Default + NumCast,
    TransformT: DecodingTransform<DataTypeT>,
    MeshDataT: MeshPredictionSchemeDataRef + Copy,
    MeshDataT::CornerTable: MeshPredictionCornerTable,
{
    fn decode_prediction_data(&mut self, buffer: &mut DecoderBuffer) -> bool {
        let mut num_orientations: i32 = 0;
        if !buffer.decode(&mut num_orientations) || num_orientations < 0 {
            return false;
        }
        self.predictor
            .resize_orientations(num_orientations as usize);
        let mut last_orientation = true;
        let mut decoder = RAnsBitDecoder::new();
        if !decoder.start_decoding(buffer) {
            return false;
        }
        for i in 0..num_orientations as usize {
            if !decoder.decode_next_bit() {
                last_orientation = !last_orientation;
            }
            self.predictor.set_orientation(i, last_orientation);
        }
        decoder.clear();
        self.base
            .base_mut()
            .transform_mut()
            .decode_transform_data(buffer)
    }
}

impl<DataTypeT, TransformT, MeshDataT>
    PredictionSchemeTypedDecoderInterface<DataTypeT, TransformT::CorrType>
    for MeshPredictionSchemeTexCoordsPortableDecoder<DataTypeT, TransformT, MeshDataT>
where
    DataTypeT: Copy + Default + NumCast,
    TransformT: DecodingTransform<DataTypeT> + Clone,
    TransformT::CorrType: Copy,
    MeshDataT: MeshPredictionSchemeDataRef + Copy,
    MeshDataT::CornerTable: MeshPredictionCornerTable,
{
    fn compute_original_values(
        &self,
        in_corr: &[TransformT::CorrType],
        out_data: &mut [DataTypeT],
        num_components: i32,
        entry_to_point_id_map: &[PointIndex],
    ) -> bool {
        if num_components != MeshPredictionSchemeTexCoordsPortablePredictor::<DataTypeT, MeshDataT>::NUM_COMPONENTS {
            return false;
        }
        let mut predictor = self.predictor.clone_with_map(entry_to_point_id_map);
        let mut transform = self.base.base().transform().clone();
        transform.init(num_components);

        let corner_map = self.base.mesh_data().data_to_corner_map();
        for p in 0..corner_map.len() {
            let corner_id = corner_map[p];
            if !predictor.compute_predicted_value::<false>(corner_id, out_data, p as i32) {
                return false;
            }
            let dst_offset = p * num_components as usize;
            transform.compute_original_value(
                predictor.predicted_value(),
                &in_corr[dst_offset..dst_offset + num_components as usize],
                &mut out_data[dst_offset..dst_offset + num_components as usize],
            );
        }
        true
    }
}
