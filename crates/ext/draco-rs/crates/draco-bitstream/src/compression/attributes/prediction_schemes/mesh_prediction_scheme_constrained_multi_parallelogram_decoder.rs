//! Constrained multi-parallelogram mesh prediction scheme decoder.
//! Reference: `_ref/draco/src/draco/compression/attributes/prediction_schemes/mesh_prediction_scheme_constrained_multi_parallelogram_decoder.h`.

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
use crate::compression::config::compression_shared::{
    bitstream_version, PredictionSchemeMethod,
};
use crate::compression::bit_coders::rans_bit_decoder::RAnsBitDecoder;
use crate::compression::attributes::prediction_schemes::mesh_prediction_scheme_constrained_multi_parallelogram_shared::{
    Mode, MAX_NUM_PARALLELOGRAMS,
};
use draco_core::attributes::geometry_attribute::GeometryAttributeType;
use draco_core::attributes::geometry_indices::PointIndex;
use draco_core::attributes::point_attribute::PointAttribute;
use draco_core::core::decoder_buffer::DecoderBuffer;
use draco_core::core::math_utils::{add_as_unsigned, AddAsUnsigned};
use draco_core::core::varint_decoding::decode_varint;

pub struct MeshPredictionSchemeConstrainedMultiParallelogramDecoder<
    DataTypeT,
    TransformT,
    MeshDataT,
> where
    TransformT: DecodingTransform<DataTypeT>,
{
    base: MeshPredictionSchemeDecoder<DataTypeT, TransformT, MeshDataT>,
    is_crease_edge: [Vec<bool>; MAX_NUM_PARALLELOGRAMS],
    selected_mode: Mode,
}

impl<DataTypeT, TransformT, MeshDataT>
    MeshPredictionSchemeConstrainedMultiParallelogramDecoder<DataTypeT, TransformT, MeshDataT>
where
    TransformT: DecodingTransform<DataTypeT>,
{
    pub fn new(attribute: &PointAttribute, transform: TransformT, mesh_data: MeshDataT) -> Self {
        Self {
            base: MeshPredictionSchemeDecoder::new(attribute, transform, mesh_data),
            is_crease_edge: [Vec::new(), Vec::new(), Vec::new(), Vec::new()],
            selected_mode: Mode::OptimalMultiParallelogram,
        }
    }
}

impl<DataTypeT, TransformT, MeshDataT> PredictionSchemeInterface
    for MeshPredictionSchemeConstrainedMultiParallelogramDecoder<DataTypeT, TransformT, MeshDataT>
where
    TransformT: DecodingTransform<DataTypeT>,
    MeshDataT: MeshPredictionSchemeDataRef,
{
    fn get_prediction_method(&self) -> PredictionSchemeMethod {
        PredictionSchemeMethod::MeshPredictionConstrainedMultiParallelogram
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
    for MeshPredictionSchemeConstrainedMultiParallelogramDecoder<DataTypeT, TransformT, MeshDataT>
where
    TransformT: DecodingTransform<DataTypeT>,
    MeshDataT: MeshPredictionSchemeDataRef,
    MeshDataT::CornerTable: MeshPredictionCornerTable,
{
    fn decode_prediction_data(&mut self, buffer: &mut DecoderBuffer) -> bool {
        if buffer.bitstream_version() < bitstream_version(2, 2) {
            let mut mode: u8 = 0;
            if !buffer.decode(&mut mode) {
                return false;
            }
            if mode != Mode::OptimalMultiParallelogram as u8 {
                return false;
            }
            self.selected_mode = Mode::OptimalMultiParallelogram;
        }

        for i in 0..MAX_NUM_PARALLELOGRAMS {
            let mut num_flags: u32 = 0;
            if !decode_varint(&mut num_flags, buffer) {
                return false;
            }
            if num_flags as usize > self.base.mesh_data().corner_table().num_corners() {
                return false;
            }
            if num_flags > 0 {
                self.is_crease_edge[i].resize(num_flags as usize, false);
                let mut decoder = RAnsBitDecoder::new();
                if !decoder.start_decoding(buffer) {
                    return false;
                }
                for j in 0..num_flags as usize {
                    self.is_crease_edge[i][j] = decoder.decode_next_bit();
                }
                decoder.clear();
            }
        }

        self.base
            .base_mut()
            .transform_mut()
            .decode_transform_data(buffer)
    }
}

impl<DataTypeT, TransformT, MeshDataT>
    PredictionSchemeTypedDecoderInterface<DataTypeT, TransformT::CorrType>
    for MeshPredictionSchemeConstrainedMultiParallelogramDecoder<DataTypeT, TransformT, MeshDataT>
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
        let mut pred_vals: [Vec<DataTypeT>; MAX_NUM_PARALLELOGRAMS] = [
            vec![DataTypeT::default(); num_components_usize],
            vec![DataTypeT::default(); num_components_usize],
            vec![DataTypeT::default(); num_components_usize],
            vec![DataTypeT::default(); num_components_usize],
        ];

        if in_corr.is_empty() {
            return true;
        }

        transform.compute_original_value(
            &pred_vals[0],
            &in_corr[0..num_components_usize],
            &mut out_data[0..num_components_usize],
        );

        let table = self.base.mesh_data().corner_table();
        let vertex_to_data_map = self.base.mesh_data().vertex_to_data_map();
        let corner_map = self.base.mesh_data().data_to_corner_map();

        let mut is_crease_edge_pos = vec![0usize; MAX_NUM_PARALLELOGRAMS];
        let mut multi_pred_vals = vec![DataTypeT::default(); num_components_usize];

        for p in 1..corner_map.len() {
            let start_corner_id = corner_map[p];
            let mut corner_id = start_corner_id;
            let mut num_parallelograms = 0usize;
            let mut first_pass = true;
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
                    &mut pred_vals[num_parallelograms],
                ) {
                    num_parallelograms += 1;
                    if num_parallelograms == MAX_NUM_PARALLELOGRAMS {
                        break;
                    }
                }
                if first_pass {
                    corner_id = table.swing_left(corner_id);
                } else {
                    corner_id = table.swing_right(corner_id);
                }
                if corner_id == start_corner_id {
                    break;
                }
                if corner_id == draco_core::attributes::geometry_indices::INVALID_CORNER_INDEX
                    && first_pass
                {
                    first_pass = false;
                    corner_id = table.swing_right(start_corner_id);
                }
            }

            let mut num_used_parallelograms = 0usize;
            if num_parallelograms > 0 {
                for c in 0..num_components_usize {
                    multi_pred_vals[c] = DataTypeT::default();
                }
                for i in 0..num_parallelograms {
                    let context = num_parallelograms - 1;
                    let pos = is_crease_edge_pos[context];
                    is_crease_edge_pos[context] += 1;
                    if self.is_crease_edge[context].len() <= pos {
                        return false;
                    }
                    let is_crease = self.is_crease_edge[context][pos];
                    if !is_crease {
                        num_used_parallelograms += 1;
                        for j in 0..num_components_usize {
                            multi_pred_vals[j] =
                                add_as_unsigned(multi_pred_vals[j], pred_vals[i][j]);
                        }
                    }
                }
            }

            let dst_slice = &mut decoded_tail[..num_components_usize];
            if num_used_parallelograms == 0 {
                let src_offset = (p - 1) * num_components_usize;
                transform.compute_original_value(
                    &decoded_prefix[src_offset..src_offset + num_components_usize],
                    &in_corr[dst_offset..dst_offset + num_components_usize],
                    dst_slice,
                );
            } else {
                let denom: DataTypeT =
                    NumCast::from(num_used_parallelograms as i32).unwrap_or_default();
                for c in 0..num_components_usize {
                    multi_pred_vals[c] = multi_pred_vals[c] / denom;
                }
                transform.compute_original_value(
                    &multi_pred_vals,
                    &in_corr[dst_offset..dst_offset + num_components_usize],
                    dst_slice,
                );
            }
        }
        true
    }
}
