//! Portable UV prediction helper for mesh texcoord prediction schemes.
//! Reference: `_ref/draco/src/draco/compression/attributes/prediction_schemes/mesh_prediction_scheme_tex_coords_portable_predictor.h`.
//!
//! This predictor is shared by encoder/decoder; we keep both code paths to
//! match the reference behavior and avoid future divergence.

use std::cmp::max;

use num_traits::NumCast;

use draco_core::attributes::geometry_indices::{CornerIndex, PointIndex};
use draco_core::attributes::point_attribute::PointAttribute;
use draco_core::core::math_utils::int_sqrt;
use draco_core::core::vector_d::VectorD;

use crate::compression::attributes::prediction_schemes::mesh_prediction_scheme_corner_table::MeshPredictionCornerTable;
use crate::compression::attributes::prediction_schemes::mesh_prediction_scheme_data::MeshPredictionSchemeDataRef;

pub struct MeshPredictionSchemeTexCoordsPortablePredictor<DataTypeT, MeshDataT>
where
    MeshDataT: MeshPredictionSchemeDataRef,
    MeshDataT::CornerTable: MeshPredictionCornerTable,
{
    pos_attribute: *const PointAttribute,
    entry_to_point_id_map: *const PointIndex,
    entry_to_point_id_map_len: usize,
    mesh_data: MeshDataT,
    predicted_value: [DataTypeT; 2],
    orientations: Vec<bool>,
}

impl<DataTypeT, MeshDataT> MeshPredictionSchemeTexCoordsPortablePredictor<DataTypeT, MeshDataT>
where
    DataTypeT: Copy + Default + NumCast,
    MeshDataT: MeshPredictionSchemeDataRef,
    MeshDataT::CornerTable: MeshPredictionCornerTable,
{
    pub const NUM_COMPONENTS: i32 = 2;

    pub fn new(mesh_data: MeshDataT) -> Self {
        Self {
            pos_attribute: std::ptr::null(),
            entry_to_point_id_map: std::ptr::null(),
            entry_to_point_id_map_len: 0,
            mesh_data,
            predicted_value: [DataTypeT::default(); 2],
            orientations: Vec::new(),
        }
    }

    pub fn set_position_attribute(&mut self, position_attribute: &PointAttribute) {
        self.pos_attribute = position_attribute as *const PointAttribute;
    }

    pub fn set_entry_to_point_id_map(&mut self, map: &[PointIndex]) {
        self.entry_to_point_id_map = map.as_ptr();
        self.entry_to_point_id_map_len = map.len();
    }

    pub fn clone_with_map(&self, map: &[PointIndex]) -> Self
    where
        MeshDataT: Copy,
    {
        Self {
            pos_attribute: self.pos_attribute,
            entry_to_point_id_map: map.as_ptr(),
            entry_to_point_id_map_len: map.len(),
            mesh_data: self.mesh_data,
            predicted_value: self.predicted_value,
            orientations: self.orientations.clone(),
        }
    }

    pub fn is_initialized(&self) -> bool {
        !self.pos_attribute.is_null()
    }

    fn entry_to_point_id_map(&self) -> &[PointIndex] {
        unsafe {
            std::slice::from_raw_parts(self.entry_to_point_id_map, self.entry_to_point_id_map_len)
        }
    }

    fn get_position_for_entry_id(&self, entry_id: i32) -> VectorD<i64, 3> {
        let point_id = self.entry_to_point_id_map()[entry_id as usize];
        let pos_att = unsafe { &*self.pos_attribute };
        let mut tmp = [0i64; 3];
        let _ = pos_att.convert_value(pos_att.mapped_index(point_id), 3, &mut tmp);
        VectorD::new3(tmp[0], tmp[1], tmp[2])
    }

    fn get_tex_coord_for_entry_id(&self, entry_id: i32, data: &[DataTypeT]) -> VectorD<i64, 2> {
        let data_offset = (entry_id as usize) * 2;
        let u = NumCast::from(data[data_offset]).unwrap_or_default();
        let v = NumCast::from(data[data_offset + 1]).unwrap_or_default();
        VectorD::new2(u, v)
    }

    pub fn predicted_value(&self) -> &[DataTypeT; 2] {
        &self.predicted_value
    }

    pub fn orientation(&self, i: usize) -> bool {
        self.orientations[i]
    }

    pub fn set_orientation(&mut self, i: usize, v: bool) {
        self.orientations[i] = v;
    }

    pub fn num_orientations(&self) -> usize {
        self.orientations.len()
    }

    pub fn resize_orientations(&mut self, num_orientations: usize) {
        self.orientations.resize(num_orientations, false);
    }

    pub fn compute_predicted_value<const IS_ENCODER: bool>(
        &mut self,
        corner_id: CornerIndex,
        data: &[DataTypeT],
        data_id: i32,
    ) -> bool {
        let corner_table = self.mesh_data.corner_table();
        let next_corner_id = corner_table.next(corner_id);
        let prev_corner_id = corner_table.previous(corner_id);
        let next_vert_id = corner_table.vertex(next_corner_id).value() as usize;
        let prev_vert_id = corner_table.vertex(prev_corner_id).value() as usize;
        let vertex_to_data_map = self.mesh_data.vertex_to_data_map();
        let next_data_id = vertex_to_data_map[next_vert_id];
        let prev_data_id = vertex_to_data_map[prev_vert_id];

        type Vec2 = VectorD<i64, 2>;

        if prev_data_id >= 0
            && next_data_id >= 0
            && prev_data_id < data_id
            && next_data_id < data_id
        {
            let n_uv = self.get_tex_coord_for_entry_id(next_data_id, data);
            let p_uv = self.get_tex_coord_for_entry_id(prev_data_id, data);
            if p_uv == n_uv {
                self.predicted_value[0] = NumCast::from(p_uv[0]).unwrap_or_default();
                self.predicted_value[1] = NumCast::from(p_uv[1]).unwrap_or_default();
                return true;
            }

            let tip_pos = self.get_position_for_entry_id(data_id);
            let next_pos = self.get_position_for_entry_id(next_data_id);
            let prev_pos = self.get_position_for_entry_id(prev_data_id);

            let pn = prev_pos - next_pos;
            let pn_norm2_squared = pn.squared_norm();
            if pn_norm2_squared != 0 {
                let cn = tip_pos - next_pos;
                let cn_dot_pn = pn.dot(&cn);

                let pn_uv = p_uv - n_uv;
                let n_uv_absmax = max(n_uv[0].abs(), n_uv[1].abs());
                if n_uv_absmax > i64::MAX / pn_norm2_squared {
                    return false;
                }
                let pn_uv_absmax = max(pn_uv[0].abs(), pn_uv[1].abs());
                if pn_uv_absmax > 0 && cn_dot_pn.abs() > i64::MAX / pn_uv_absmax {
                    return false;
                }
                let x_uv = n_uv * pn_norm2_squared + pn_uv * cn_dot_pn;

                let pn_absmax = max(max(pn[0].abs(), pn[1].abs()), pn[2].abs());
                if pn_absmax > 0 && cn_dot_pn.abs() > i64::MAX / pn_absmax {
                    return false;
                }
                let x_pos = next_pos + (pn * cn_dot_pn) / pn_norm2_squared;
                let cx_norm2_squared = (tip_pos - x_pos).squared_norm() as u64;

                let mut cx_uv = Vec2::new2(pn_uv[1], -pn_uv[0]);
                let norm_squared = int_sqrt(cx_norm2_squared.wrapping_mul(pn_norm2_squared as u64));
                cx_uv = cx_uv * (norm_squared as i64);

                let predicted_uv = if IS_ENCODER {
                    let predicted_uv_0 = (x_uv + cx_uv) / pn_norm2_squared;
                    let predicted_uv_1 = (x_uv - cx_uv) / pn_norm2_squared;
                    let c_uv = self.get_tex_coord_for_entry_id(data_id, data);
                    if (c_uv - predicted_uv_0).squared_norm()
                        < (c_uv - predicted_uv_1).squared_norm()
                    {
                        self.orientations.push(true);
                        predicted_uv_0
                    } else {
                        self.orientations.push(false);
                        predicted_uv_1
                    }
                } else {
                    // Parity: C++ returns false when orientations exhausted (no fallback).
                    let orientation = match self.orientations.pop() {
                        Some(o) => o,
                        None => return false,
                    };
                    let x_u0 = x_uv[0] as u64;
                    let x_u1 = x_uv[1] as u64;
                    let cx_u0 = cx_uv[0] as u64;
                    let cx_u1 = cx_uv[1] as u64;
                    let denom = pn_norm2_squared as u64;
                    let pred0_u = if orientation {
                        x_u0.wrapping_add(cx_u0) / denom
                    } else {
                        x_u0.wrapping_sub(cx_u0) / denom
                    };
                    let pred1_u = if orientation {
                        x_u1.wrapping_add(cx_u1) / denom
                    } else {
                        x_u1.wrapping_sub(cx_u1) / denom
                    };
                    Vec2::new2(pred0_u as i64, pred1_u as i64)
                };

                self.predicted_value[0] = NumCast::from(predicted_uv[0]).unwrap_or_default();
                self.predicted_value[1] = NumCast::from(predicted_uv[1]).unwrap_or_default();
                return true;
            }
        }

        // Fallback: use available neighbor or last encoded value.
        let data_offset = if next_data_id < data_id {
            next_data_id as usize * 2
        } else if prev_data_id < data_id {
            prev_data_id as usize * 2
        } else if data_id > 0 {
            (data_id as usize - 1) * 2
        } else {
            self.predicted_value[0] = DataTypeT::default();
            self.predicted_value[1] = DataTypeT::default();
            return true;
        };
        self.predicted_value[0] = data[data_offset];
        self.predicted_value[1] = data[data_offset + 1];
        true
    }
}
