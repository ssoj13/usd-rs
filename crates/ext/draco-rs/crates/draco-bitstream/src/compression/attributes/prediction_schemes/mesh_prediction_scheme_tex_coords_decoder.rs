//! Deprecated texcoords mesh prediction scheme decoder.
//! Reference: `_ref/draco/src/draco/compression/attributes/prediction_schemes/mesh_prediction_scheme_tex_coords_decoder.h`.
//!
//! This scheme is kept for backwards compatibility with older Draco bitstreams.

use std::any::TypeId;

use num_traits::NumCast;

use crate::compression::attributes::prediction_schemes::mesh_prediction_scheme_corner_table::MeshPredictionCornerTable;
use crate::compression::attributes::prediction_schemes::mesh_prediction_scheme_data::MeshPredictionSchemeDataRef;
use crate::compression::attributes::prediction_schemes::mesh_prediction_scheme_decoder::MeshPredictionSchemeDecoder;
use crate::compression::attributes::prediction_schemes::prediction_scheme_decoder_interface::{
    PredictionSchemeDecoderInterface, PredictionSchemeTypedDecoderInterface,
};
use crate::compression::attributes::prediction_schemes::prediction_scheme_decoding_transform::DecodingTransform;
use crate::compression::attributes::prediction_schemes::prediction_scheme_interface::PredictionSchemeInterface;
use crate::compression::bit_coders::rans_bit_decoder::RAnsBitDecoder;
use crate::compression::config::compression_shared::{bitstream_version, PredictionSchemeMethod};
use draco_core::attributes::geometry_attribute::GeometryAttributeType;
use draco_core::attributes::geometry_indices::{CornerIndex, PointIndex};
use draco_core::attributes::point_attribute::PointAttribute;
use draco_core::core::decoder_buffer::DecoderBuffer;
use draco_core::core::varint_decoding::decode_varint;
use draco_core::core::vector_d::{Vector2f, Vector3f};

pub struct MeshPredictionSchemeTexCoordsDecoder<DataTypeT, TransformT, MeshDataT>
where
    TransformT: DecodingTransform<DataTypeT>,
    MeshDataT: MeshPredictionSchemeDataRef,
    MeshDataT::CornerTable: MeshPredictionCornerTable,
{
    base: MeshPredictionSchemeDecoder<DataTypeT, TransformT, MeshDataT>,
    pos_attribute: *const PointAttribute,
    orientations: Vec<bool>,
    version: u16,
}

impl<DataTypeT, TransformT, MeshDataT>
    MeshPredictionSchemeTexCoordsDecoder<DataTypeT, TransformT, MeshDataT>
where
    DataTypeT: Copy + Default + NumCast + 'static,
    TransformT: DecodingTransform<DataTypeT>,
    MeshDataT: MeshPredictionSchemeDataRef,
    MeshDataT::CornerTable: MeshPredictionCornerTable,
{
    pub fn new(
        attribute: &PointAttribute,
        transform: TransformT,
        mesh_data: MeshDataT,
        version: u16,
    ) -> Self {
        Self {
            base: MeshPredictionSchemeDecoder::new(attribute, transform, mesh_data),
            pos_attribute: std::ptr::null(),
            orientations: Vec::new(),
            version,
        }
    }

    fn get_position_for_entry_id(
        &self,
        entry_id: i32,
        entry_to_point_id_map: &[PointIndex],
    ) -> Vector3f {
        let point_id = entry_to_point_id_map[entry_id as usize];
        let pos_att = unsafe { &*self.pos_attribute };
        let mut tmp = [0f32; 3];
        let _ = pos_att.convert_value(pos_att.mapped_index(point_id), 3, &mut tmp);
        Vector3f::new3(tmp[0], tmp[1], tmp[2])
    }

    fn get_tex_coord_for_entry_id(
        &self,
        entry_id: i32,
        data: &[DataTypeT],
        num_components: i32,
    ) -> Vector2f {
        let data_offset = entry_id as usize * num_components as usize;
        let u = NumCast::from(data[data_offset]).unwrap_or_default();
        let v = NumCast::from(data[data_offset + 1]).unwrap_or_default();
        Vector2f::new2(u, v)
    }

    fn is_float_type() -> bool {
        TypeId::of::<DataTypeT>() == TypeId::of::<f32>()
            || TypeId::of::<DataTypeT>() == TypeId::of::<f64>()
    }

    fn clamp_float_to_i32(value: f32) -> i32 {
        if value.is_nan() {
            return i32::MIN;
        }
        let v = value as f64;
        if v > i32::MAX as f64 || v < i32::MIN as f64 {
            return i32::MIN;
        }
        value as i32
    }

    fn round_float_to_i32(value: f32) -> i32 {
        let v = (value as f64 + 0.5).floor();
        if v.is_nan() || v > i32::MAX as f64 || v < i32::MIN as f64 {
            return i32::MIN;
        }
        v as i32
    }

    fn compute_predicted_value(
        &self,
        corner_id: CornerIndex,
        data: &[DataTypeT],
        data_id: i32,
        entry_to_point_id_map: &[PointIndex],
        num_components: i32,
        orientations: &mut Vec<bool>,
        predicted_value: &mut [DataTypeT],
    ) -> bool {
        let corner_table = self.base.mesh_data().corner_table();
        let next_corner_id = corner_table.next(corner_id);
        let prev_corner_id = corner_table.previous(corner_id);

        let next_vert_id = corner_table.vertex(next_corner_id).value() as usize;
        let prev_vert_id = corner_table.vertex(prev_corner_id).value() as usize;

        let vertex_to_data_map = self.base.mesh_data().vertex_to_data_map();
        let next_data_id = vertex_to_data_map[next_vert_id];
        let prev_data_id = vertex_to_data_map[prev_vert_id];

        if prev_data_id < data_id && next_data_id < data_id {
            let n_uv = self.get_tex_coord_for_entry_id(next_data_id, data, num_components);
            let p_uv = self.get_tex_coord_for_entry_id(prev_data_id, data, num_components);
            if p_uv == n_uv {
                let u = Self::clamp_float_to_i32(p_uv[0]);
                let v = Self::clamp_float_to_i32(p_uv[1]);
                predicted_value[0] = NumCast::from(u).unwrap_or_default();
                predicted_value[1] = NumCast::from(v).unwrap_or_default();
                return true;
            }

            let tip_pos = self.get_position_for_entry_id(data_id, entry_to_point_id_map);
            let next_pos = self.get_position_for_entry_id(next_data_id, entry_to_point_id_map);
            let prev_pos = self.get_position_for_entry_id(prev_data_id, entry_to_point_id_map);

            let pn = prev_pos - next_pos;
            let cn = tip_pos - next_pos;
            let pn_norm2_squared = pn.squared_norm();
            let (s, t) = if self.version < bitstream_version(1, 2) || pn_norm2_squared > 0.0 {
                let s_val = pn.dot(&cn) / pn_norm2_squared;
                let t_val = ((cn - pn * s_val).squared_norm() / pn_norm2_squared).sqrt();
                (s_val, t_val)
            } else {
                (0.0, 0.0)
            };

            let pn_uv = p_uv - n_uv;
            let pnus = pn_uv[0] * s + n_uv[0];
            let pnut = pn_uv[0] * t;
            let pnvs = pn_uv[1] * s + n_uv[1];
            let pnvt = pn_uv[1] * t;

            if orientations.is_empty() {
                return false;
            }
            let orientation = orientations.pop().unwrap();
            let predicted_uv = if orientation {
                Vector2f::new2(pnus - pnvt, pnvs + pnut)
            } else {
                Vector2f::new2(pnus + pnvt, pnvs - pnut)
            };

            if !Self::is_float_type() {
                let u = Self::round_float_to_i32(predicted_uv[0]);
                let v = Self::round_float_to_i32(predicted_uv[1]);
                predicted_value[0] = NumCast::from(u).unwrap_or_default();
                predicted_value[1] = NumCast::from(v).unwrap_or_default();
            } else {
                let u = predicted_uv[0] as i32;
                let v = predicted_uv[1] as i32;
                predicted_value[0] = NumCast::from(u).unwrap_or_default();
                predicted_value[1] = NumCast::from(v).unwrap_or_default();
            }
            return true;
        }

        let data_offset = if next_data_id < data_id {
            next_data_id as usize * num_components as usize
        } else if prev_data_id < data_id {
            prev_data_id as usize * num_components as usize
        } else if data_id > 0 {
            (data_id as usize - 1) * num_components as usize
        } else {
            predicted_value[0] = DataTypeT::default();
            predicted_value[1] = DataTypeT::default();
            return true;
        };
        predicted_value[0] = data[data_offset];
        predicted_value[1] = data[data_offset + 1];
        true
    }
}

impl<DataTypeT, TransformT, MeshDataT> PredictionSchemeInterface
    for MeshPredictionSchemeTexCoordsDecoder<DataTypeT, TransformT, MeshDataT>
where
    DataTypeT: Copy + Default + NumCast + 'static,
    TransformT: DecodingTransform<DataTypeT>,
    MeshDataT: MeshPredictionSchemeDataRef,
    MeshDataT::CornerTable: MeshPredictionCornerTable,
{
    fn get_prediction_method(&self) -> PredictionSchemeMethod {
        PredictionSchemeMethod::MeshPredictionTexCoordsDeprecated
    }

    fn get_attribute(&self) -> &PointAttribute {
        self.base.base().attribute()
    }

    fn is_initialized(&self) -> bool {
        !self.pos_attribute.is_null() && self.base.mesh_data().is_initialized()
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
        self.pos_attribute = att as *const PointAttribute;
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
    for MeshPredictionSchemeTexCoordsDecoder<DataTypeT, TransformT, MeshDataT>
where
    DataTypeT: Copy + Default + NumCast + 'static,
    TransformT: DecodingTransform<DataTypeT>,
    MeshDataT: MeshPredictionSchemeDataRef,
    MeshDataT::CornerTable: MeshPredictionCornerTable,
{
    fn decode_prediction_data(&mut self, buffer: &mut DecoderBuffer) -> bool {
        let mut num_orientations: u32 = 0;
        if buffer.bitstream_version() < bitstream_version(2, 2) {
            if !buffer.decode(&mut num_orientations) {
                return false;
            }
        } else if !decode_varint(&mut num_orientations, buffer) {
            return false;
        }
        if num_orientations == 0 {
            return false;
        }
        if num_orientations as usize > self.base.mesh_data().corner_table().num_corners() {
            return false;
        }
        self.orientations.resize(num_orientations as usize, false);
        let mut last_orientation = true;
        let mut decoder = RAnsBitDecoder::new();
        if !decoder.start_decoding(buffer) {
            return false;
        }
        for i in 0..num_orientations as usize {
            if !decoder.decode_next_bit() {
                last_orientation = !last_orientation;
            }
            self.orientations[i] = last_orientation;
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
    for MeshPredictionSchemeTexCoordsDecoder<DataTypeT, TransformT, MeshDataT>
where
    DataTypeT: Copy + Default + NumCast + 'static,
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
        entry_to_point_id_map: &[PointIndex],
    ) -> bool {
        if num_components != 2 {
            return false;
        }
        if self.pos_attribute.is_null() {
            return false;
        }

        let mut transform = self.base.base().transform().clone();
        transform.init(num_components);

        let corner_map = self.base.mesh_data().data_to_corner_map();
        let mut predicted_value = vec![DataTypeT::default(); num_components as usize];
        let mut orientations = self.orientations.clone();

        for p in 0..corner_map.len() {
            let corner_id = corner_map[p];
            if !self.compute_predicted_value(
                corner_id,
                out_data,
                p as i32,
                entry_to_point_id_map,
                num_components,
                &mut orientations,
                &mut predicted_value,
            ) {
                return false;
            }
            let dst_offset = p * num_components as usize;
            transform.compute_original_value(
                &predicted_value,
                &in_corr[dst_offset..dst_offset + num_components as usize],
                &mut out_data[dst_offset..dst_offset + num_components as usize],
            );
        }
        true
    }
}
