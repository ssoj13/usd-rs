//! Geometric normal mesh prediction scheme decoder.
//! Reference: `_ref/draco/src/draco/compression/attributes/prediction_schemes/mesh_prediction_scheme_geometric_normal_decoder.h`.

use std::cell::{Cell, RefCell};

use num_traits::{NumCast, ToPrimitive};

use crate::compression::attributes::prediction_schemes::mesh_prediction_scheme_data::
    MeshPredictionSchemeDataRef;
use crate::compression::attributes::prediction_schemes::mesh_prediction_scheme_decoder::
    MeshPredictionSchemeDecoder;
use crate::compression::attributes::prediction_schemes::mesh_prediction_scheme_geometric_normal_predictor_area::
    MeshPredictionSchemeGeometricNormalPredictorArea;
use crate::compression::attributes::prediction_schemes::prediction_scheme_decoder_interface::{
    PredictionSchemeDecoderInterface, PredictionSchemeTypedDecoderInterface,
};
use crate::compression::attributes::prediction_schemes::prediction_scheme_decoding_transform::
    DecodingTransform;
use crate::compression::attributes::prediction_schemes::prediction_scheme_interface::
    PredictionSchemeInterface;
use crate::compression::bit_coders::rans_bit_decoder::RAnsBitDecoder;
use crate::compression::config::compression_shared::{
    bitstream_version, NormalPredictionMode, PredictionSchemeMethod,
};
use draco_core::attributes::geometry_attribute::GeometryAttributeType;
use draco_core::attributes::geometry_indices::PointIndex;
use draco_core::attributes::point_attribute::PointAttribute;
use draco_core::compression::attributes::normal_compression_utils::OctahedronToolBox;
use draco_core::core::decoder_buffer::DecoderBuffer;

pub struct MeshPredictionSchemeGeometricNormalDecoder<DataTypeT, TransformT, MeshDataT>
where
    TransformT: DecodingTransform<DataTypeT>,
    MeshDataT: MeshPredictionSchemeDataRef + Copy,
    MeshDataT::CornerTable: draco_core::mesh::corner_table_iterators::CornerTableTraversal,
{
    base: MeshPredictionSchemeDecoder<DataTypeT, TransformT, MeshDataT>,
    predictor: MeshPredictionSchemeGeometricNormalPredictorArea<DataTypeT, TransformT, MeshDataT>,
    octahedron_tool_box: Cell<OctahedronToolBox>,
    flip_normal_bit_decoder: RefCell<RAnsBitDecoder>,
}

impl<DataTypeT, TransformT, MeshDataT>
    MeshPredictionSchemeGeometricNormalDecoder<DataTypeT, TransformT, MeshDataT>
where
    DataTypeT: Copy + Default + NumCast + ToPrimitive,
    TransformT: DecodingTransform<DataTypeT>,
    MeshDataT: MeshPredictionSchemeDataRef + Copy,
    MeshDataT::CornerTable: draco_core::mesh::corner_table_iterators::CornerTableTraversal,
{
    pub fn new(attribute: &PointAttribute, transform: TransformT, mesh_data: MeshDataT) -> Self {
        Self {
            base: MeshPredictionSchemeDecoder::new(attribute, transform, mesh_data),
            predictor: MeshPredictionSchemeGeometricNormalPredictorArea::new(mesh_data),
            octahedron_tool_box: Cell::new(OctahedronToolBox::new()),
            flip_normal_bit_decoder: RefCell::new(RAnsBitDecoder::new()),
        }
    }

    fn set_quantization_bits(&self, q: i32) -> bool {
        let mut tool_box = self.octahedron_tool_box.get();
        let ok = tool_box.set_quantization_bits(q);
        self.octahedron_tool_box.set(tool_box);
        ok
    }
}

impl<DataTypeT, TransformT, MeshDataT> PredictionSchemeInterface
    for MeshPredictionSchemeGeometricNormalDecoder<DataTypeT, TransformT, MeshDataT>
where
    DataTypeT: Copy + Default + NumCast + ToPrimitive,
    TransformT: DecodingTransform<DataTypeT>,
    MeshDataT: MeshPredictionSchemeDataRef + Copy,
    MeshDataT::CornerTable: draco_core::mesh::corner_table_iterators::CornerTableTraversal,
{
    fn get_prediction_method(&self) -> PredictionSchemeMethod {
        PredictionSchemeMethod::MeshPredictionGeometricNormal
    }

    fn get_attribute(&self) -> &PointAttribute {
        self.base.base().attribute()
    }

    fn is_initialized(&self) -> bool {
        self.predictor.is_initialized()
            && self.base.mesh_data().is_initialized()
            && self.octahedron_tool_box.get().is_initialized()
    }

    fn get_num_parent_attributes(&self) -> i32 {
        1
    }

    fn get_parent_attribute_type(&self, _i: i32) -> GeometryAttributeType {
        GeometryAttributeType::Position
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
    for MeshPredictionSchemeGeometricNormalDecoder<DataTypeT, TransformT, MeshDataT>
where
    DataTypeT: Copy + Default + NumCast + ToPrimitive,
    TransformT: DecodingTransform<DataTypeT>,
    MeshDataT: MeshPredictionSchemeDataRef + Copy,
    MeshDataT::CornerTable: draco_core::mesh::corner_table_iterators::CornerTableTraversal,
{
    fn decode_prediction_data(&mut self, buffer: &mut DecoderBuffer) -> bool {
        if !self
            .base
            .base_mut()
            .transform_mut()
            .decode_transform_data(buffer)
        {
            return false;
        }

        if buffer.bitstream_version() < bitstream_version(2, 2) {
            let mut prediction_mode: u8 = 0;
            if !buffer.decode(&mut prediction_mode) {
                return false;
            }
            let mode = match prediction_mode {
                0 => NormalPredictionMode::OneTriangle,
                1 => NormalPredictionMode::TriangleArea,
                _ => return false,
            };
            if !self.predictor.set_normal_prediction_mode(mode) {
                return false;
            }
        }

        if !self
            .flip_normal_bit_decoder
            .borrow_mut()
            .start_decoding(buffer)
        {
            return false;
        }
        true
    }
}

impl<DataTypeT, TransformT, MeshDataT>
    PredictionSchemeTypedDecoderInterface<DataTypeT, TransformT::CorrType>
    for MeshPredictionSchemeGeometricNormalDecoder<DataTypeT, TransformT, MeshDataT>
where
    DataTypeT: Copy + Default + NumCast + ToPrimitive,
    TransformT: DecodingTransform<DataTypeT> + Clone,
    TransformT::CorrType: Copy,
    MeshDataT: MeshPredictionSchemeDataRef + Copy,
    MeshDataT::CornerTable: draco_core::mesh::corner_table_iterators::CornerTableTraversal,
{
    fn compute_original_values(
        &self,
        in_corr: &[TransformT::CorrType],
        out_data: &mut [DataTypeT],
        num_components: i32,
        entry_to_point_id_map: &[PointIndex],
    ) -> bool {
        if !self.set_quantization_bits(self.base.base().transform().quantization_bits()) {
            return false;
        }
        let predictor = self.predictor.clone_with_map(entry_to_point_id_map);
        if !predictor.is_initialized() || !self.base.mesh_data().is_initialized() {
            return false;
        }

        if num_components != 2 {
            return false;
        }

        let mut transform = self.base.base().transform().clone();
        transform.init(num_components);

        let corner_map = self.base.mesh_data().data_to_corner_map();
        let mut pred_normal_3d = [DataTypeT::default(); 3];
        let mut pred_normal_3d_i32 = [0i32; 3];
        let mut pred_normal_oct_i32 = [0i32; 2];
        let mut pred_normal_oct = [DataTypeT::default(); 2];
        let mut flip_decoder = self.flip_normal_bit_decoder.borrow_mut();
        let octahedron_tool_box = self.octahedron_tool_box.get();

        for data_id in 0..corner_map.len() {
            let corner_id = corner_map[data_id];
            predictor.compute_predicted_value(corner_id, &mut pred_normal_3d);

            for i in 0..3 {
                pred_normal_3d_i32[i] = NumCast::from(pred_normal_3d[i]).unwrap_or_default();
            }
            octahedron_tool_box.canonicalize_integer_vector(&mut pred_normal_3d_i32);
            if flip_decoder.decode_next_bit() {
                pred_normal_3d_i32 = [
                    -pred_normal_3d_i32[0],
                    -pred_normal_3d_i32[1],
                    -pred_normal_3d_i32[2],
                ];
            }
            let mut pred_x = 0i32;
            let mut pred_y = 0i32;
            octahedron_tool_box.integer_vector_to_quantized_octahedral_coords(
                &pred_normal_3d_i32,
                &mut pred_x,
                &mut pred_y,
            );
            pred_normal_oct_i32[0] = pred_x;
            pred_normal_oct_i32[1] = pred_y;
            pred_normal_oct[0] = NumCast::from(pred_normal_oct_i32[0]).unwrap_or_default();
            pred_normal_oct[1] = NumCast::from(pred_normal_oct_i32[1]).unwrap_or_default();

            let data_offset = data_id * 2;
            transform.compute_original_value(
                &pred_normal_oct,
                &in_corr[data_offset..data_offset + 2],
                &mut out_data[data_offset..data_offset + 2],
            );
        }

        flip_decoder.clear();
        self.octahedron_tool_box.set(octahedron_tool_box);
        true
    }
}
