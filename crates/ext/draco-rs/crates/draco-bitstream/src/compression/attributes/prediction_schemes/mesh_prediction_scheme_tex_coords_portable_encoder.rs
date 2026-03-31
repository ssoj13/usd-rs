//! Portable texcoords mesh prediction scheme encoder.
//! Reference: `_ref/draco/src/draco/compression/attributes/prediction_schemes/mesh_prediction_scheme_tex_coords_portable_encoder.h`.

use num_traits::NumCast;

use crate::compression::attributes::prediction_schemes::mesh_prediction_scheme_corner_table::MeshPredictionCornerTable;
use crate::compression::attributes::prediction_schemes::mesh_prediction_scheme_data::MeshPredictionSchemeDataRef;
use crate::compression::attributes::prediction_schemes::mesh_prediction_scheme_tex_coords_portable_predictor::MeshPredictionSchemeTexCoordsPortablePredictor;
use crate::compression::attributes::prediction_schemes::prediction_scheme_encoder::PredictionSchemeEncoder;
use crate::compression::attributes::prediction_schemes::prediction_scheme_encoder_interface::{
    PredictionSchemeEncoderInterface, PredictionSchemeTypedEncoderInterface,
};
use crate::compression::attributes::prediction_schemes::prediction_scheme_encoding_transform::EncodingTransform;
use crate::compression::attributes::prediction_schemes::prediction_scheme_interface::PredictionSchemeInterface;
use crate::compression::bit_coders::rans_bit_encoder::RAnsBitEncoder;
use crate::compression::config::compression_shared::{
    PredictionSchemeMethod, PredictionSchemeTransformType,
};
use draco_core::attributes::geometry_attribute::GeometryAttributeType;
use draco_core::attributes::geometry_indices::PointIndex;
use draco_core::attributes::point_attribute::PointAttribute;
use draco_core::core::encoder_buffer::EncoderBuffer;

pub struct MeshPredictionSchemeTexCoordsPortableEncoder<DataTypeT, TransformT, MeshDataT>
where
    TransformT: EncodingTransform<DataTypeT>,
    MeshDataT: MeshPredictionSchemeDataRef + Copy,
    MeshDataT::CornerTable: MeshPredictionCornerTable,
{
    base: PredictionSchemeEncoder<DataTypeT, TransformT>,
    mesh_data: MeshDataT,
    predictor: MeshPredictionSchemeTexCoordsPortablePredictor<DataTypeT, MeshDataT>,
}

impl<DataTypeT, TransformT, MeshDataT>
    MeshPredictionSchemeTexCoordsPortableEncoder<DataTypeT, TransformT, MeshDataT>
where
    DataTypeT: Copy + Default + NumCast,
    TransformT: EncodingTransform<DataTypeT>,
    MeshDataT: MeshPredictionSchemeDataRef + Copy,
    MeshDataT::CornerTable: MeshPredictionCornerTable,
{
    pub fn new(attribute: &PointAttribute, transform: TransformT, mesh_data: MeshDataT) -> Self {
        let predictor = MeshPredictionSchemeTexCoordsPortablePredictor::new(mesh_data);
        Self {
            base: PredictionSchemeEncoder::new(attribute, transform),
            mesh_data,
            predictor,
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
    for MeshPredictionSchemeTexCoordsPortableEncoder<DataTypeT, TransformT, MeshDataT>
where
    DataTypeT: Copy + Default + NumCast,
    TransformT: EncodingTransform<DataTypeT>,
    MeshDataT: MeshPredictionSchemeDataRef + Copy,
    MeshDataT::CornerTable: MeshPredictionCornerTable,
{
    fn get_prediction_method(&self) -> PredictionSchemeMethod {
        PredictionSchemeMethod::MeshPredictionTexCoordsPortable
    }

    fn get_attribute(&self) -> &PointAttribute {
        self.base.attribute()
    }

    fn is_initialized(&self) -> bool {
        self.predictor.is_initialized() && self.mesh_data.is_initialized()
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
        self.base.transform().are_corrections_positive()
    }

    fn get_transform_type(&self) -> PredictionSchemeTransformType {
        self.base.transform().get_type()
    }
}

impl<DataTypeT, TransformT, MeshDataT> PredictionSchemeEncoderInterface
    for MeshPredictionSchemeTexCoordsPortableEncoder<DataTypeT, TransformT, MeshDataT>
where
    DataTypeT: Copy + Default + NumCast,
    TransformT: EncodingTransform<DataTypeT>,
    MeshDataT: MeshPredictionSchemeDataRef + Copy,
    MeshDataT::CornerTable: MeshPredictionCornerTable,
{
    fn encode_prediction_data(&self, buffer: &mut EncoderBuffer) -> bool {
        let num_orientations = self.predictor.num_orientations() as i32;
        if !buffer.encode(num_orientations) {
            return false;
        }
        let mut last_orientation = true;
        let mut encoder = RAnsBitEncoder::new();
        encoder.start_encoding();
        for i in 0..num_orientations as usize {
            let orientation = self.predictor.orientation(i);
            encoder.encode_bit(orientation == last_orientation);
            last_orientation = orientation;
        }
        encoder.end_encoding(buffer);
        self.base.transform().encode_transform_data(buffer)
    }
}

impl<DataTypeT, TransformT, MeshDataT>
    PredictionSchemeTypedEncoderInterface<DataTypeT, TransformT::CorrType>
    for MeshPredictionSchemeTexCoordsPortableEncoder<DataTypeT, TransformT, MeshDataT>
where
    DataTypeT: Copy + Default + NumCast,
    TransformT: EncodingTransform<DataTypeT> + Clone,
    TransformT::CorrType: Copy,
    MeshDataT: MeshPredictionSchemeDataRef + Copy,
    MeshDataT::CornerTable: MeshPredictionCornerTable,
{
    fn compute_correction_values(
        &mut self,
        in_data: &[DataTypeT],
        out_corr: &mut [TransformT::CorrType],
        num_components: i32,
        entry_to_point_id_map: &[PointIndex],
    ) -> bool {
        if num_components != MeshPredictionSchemeTexCoordsPortablePredictor::<DataTypeT, MeshDataT>::NUM_COMPONENTS {
            return false;
        }
        self.predictor
            .set_entry_to_point_id_map(entry_to_point_id_map);
        self.base_mut()
            .transform_mut()
            .init(in_data, in_data.len() as i32, num_components);
        let transform = self.base.transform();
        let num_components_usize = num_components as usize;

        let corner_map = self.mesh_data.data_to_corner_map();
        let num_entries = corner_map.len();
        for p in (0..num_entries).rev() {
            let corner_id = corner_map[p];
            if !self
                .predictor
                .compute_predicted_value::<true>(corner_id, in_data, p as i32)
            {
                return false;
            }
            let dst_offset = p * num_components_usize;
            transform.compute_correction(
                &in_data[dst_offset..dst_offset + num_components_usize],
                self.predictor.predicted_value(),
                &mut out_corr[dst_offset..dst_offset + num_components_usize],
            );
        }
        true
    }
}
