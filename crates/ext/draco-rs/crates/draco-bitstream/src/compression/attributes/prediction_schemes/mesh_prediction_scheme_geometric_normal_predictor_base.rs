//! Base helper for geometric normal mesh prediction.
//! Reference: `_ref/draco/src/draco/compression/attributes/prediction_schemes/mesh_prediction_scheme_geometric_normal_predictor_base.h`.

use std::marker::PhantomData;

use num_traits::NumCast;

use crate::compression::attributes::prediction_schemes::mesh_prediction_scheme_data::MeshPredictionSchemeDataRef;
use crate::compression::config::compression_shared::NormalPredictionMode;
use draco_core::attributes::geometry_indices::{CornerIndex, PointIndex};
use draco_core::attributes::point_attribute::PointAttribute;
use draco_core::core::vector_d::VectorD;
use draco_core::mesh::corner_table_iterators::CornerTableTraversal;

#[derive(Clone, Copy)]
pub struct MeshPredictionSchemeGeometricNormalPredictorBase<DataTypeT, TransformT, MeshDataT>
where
    MeshDataT: MeshPredictionSchemeDataRef + Copy,
    MeshDataT::CornerTable: CornerTableTraversal,
{
    pos_attribute: *const PointAttribute,
    entry_to_point_id_map: *const PointIndex,
    entry_to_point_id_map_len: usize,
    mesh_data: MeshDataT,
    normal_prediction_mode: NormalPredictionMode,
    _phantom: PhantomData<(DataTypeT, TransformT)>,
}

impl<DataTypeT, TransformT, MeshDataT>
    MeshPredictionSchemeGeometricNormalPredictorBase<DataTypeT, TransformT, MeshDataT>
where
    DataTypeT: Copy + Default + NumCast,
    MeshDataT: MeshPredictionSchemeDataRef + Copy,
    MeshDataT::CornerTable: CornerTableTraversal,
{
    pub fn new(mesh_data: MeshDataT) -> Self {
        Self {
            pos_attribute: std::ptr::null(),
            entry_to_point_id_map: std::ptr::null(),
            entry_to_point_id_map_len: 0,
            mesh_data,
            normal_prediction_mode: NormalPredictionMode::TriangleArea,
            _phantom: PhantomData,
        }
    }

    pub fn set_position_attribute(&mut self, position_attribute: &PointAttribute) {
        self.pos_attribute = position_attribute as *const PointAttribute;
    }

    pub fn set_entry_to_point_id_map(&mut self, map: &[PointIndex]) {
        self.entry_to_point_id_map = map.as_ptr();
        self.entry_to_point_id_map_len = map.len();
    }

    pub fn is_initialized(&self) -> bool {
        !self.pos_attribute.is_null() && !self.entry_to_point_id_map.is_null()
    }

    pub fn normal_prediction_mode(&self) -> NormalPredictionMode {
        self.normal_prediction_mode
    }

    pub fn set_normal_prediction_mode(&mut self, mode: NormalPredictionMode) {
        self.normal_prediction_mode = mode;
    }

    pub fn mesh_data(&self) -> &MeshDataT {
        &self.mesh_data
    }

    fn entry_to_point_id_map(&self) -> &[PointIndex] {
        unsafe {
            std::slice::from_raw_parts(self.entry_to_point_id_map, self.entry_to_point_id_map_len)
        }
    }

    pub fn get_position_for_data_id(&self, data_id: i32) -> VectorD<i64, 3> {
        let point_id = self.entry_to_point_id_map()[data_id as usize];
        let pos_att = unsafe { &*self.pos_attribute };
        let mut tmp = [0i64; 3];
        let _ = pos_att.convert_value(pos_att.mapped_index(point_id), 3, &mut tmp);
        VectorD::new3(tmp[0], tmp[1], tmp[2])
    }

    pub fn get_position_for_corner(&self, corner_id: CornerIndex) -> VectorD<i64, 3> {
        let corner_table = self.mesh_data.corner_table();
        let vert_id = corner_table.vertex(corner_id).value() as usize;
        let data_id = self.mesh_data.vertex_to_data_map()[vert_id];
        self.get_position_for_data_id(data_id)
    }

    pub fn get_octahedral_coord_for_data_id(
        &self,
        data_id: i32,
        data: &[DataTypeT],
    ) -> VectorD<i32, 2> {
        let data_offset = data_id as usize * 2;
        let u: i32 = NumCast::from(data[data_offset]).unwrap_or_default();
        let v: i32 = NumCast::from(data[data_offset + 1]).unwrap_or_default();
        VectorD::new2(u, v)
    }
}
