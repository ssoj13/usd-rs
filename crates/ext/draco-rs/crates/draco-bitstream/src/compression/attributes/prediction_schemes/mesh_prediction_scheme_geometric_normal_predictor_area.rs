//! Area-weighted geometric normal prediction.
//! Reference: `_ref/draco/src/draco/compression/attributes/prediction_schemes/mesh_prediction_scheme_geometric_normal_predictor_area.h`.

use num_traits::NumCast;

use crate::compression::attributes::prediction_schemes::mesh_prediction_scheme_geometric_normal_predictor_base::
    MeshPredictionSchemeGeometricNormalPredictorBase;
use crate::compression::attributes::prediction_schemes::mesh_prediction_scheme_data::
    MeshPredictionSchemeDataRef;
use crate::compression::config::compression_shared::NormalPredictionMode;
use draco_core::attributes::geometry_indices::CornerIndex;
use draco_core::core::vector_d::{cross_product, VectorD};
use draco_core::mesh::corner_table_iterators::{CornerTableTraversal, VertexCornersIterator};

pub struct MeshPredictionSchemeGeometricNormalPredictorArea<DataTypeT, TransformT, MeshDataT>
where
    MeshDataT: MeshPredictionSchemeDataRef + Copy,
    MeshDataT::CornerTable: CornerTableTraversal,
{
    base: MeshPredictionSchemeGeometricNormalPredictorBase<DataTypeT, TransformT, MeshDataT>,
}

impl<DataTypeT, TransformT, MeshDataT>
    MeshPredictionSchemeGeometricNormalPredictorArea<DataTypeT, TransformT, MeshDataT>
where
    DataTypeT: Copy + Default + NumCast,
    MeshDataT: MeshPredictionSchemeDataRef + Copy,
    MeshDataT::CornerTable: CornerTableTraversal,
{
    pub fn new(mesh_data: MeshDataT) -> Self {
        let mut base = MeshPredictionSchemeGeometricNormalPredictorBase::new(mesh_data);
        base.set_normal_prediction_mode(NormalPredictionMode::TriangleArea);
        Self { base }
    }

    pub fn set_position_attribute(
        &mut self,
        position_attribute: &draco_core::attributes::point_attribute::PointAttribute,
    ) {
        self.base.set_position_attribute(position_attribute);
    }

    pub fn set_entry_to_point_id_map(
        &mut self,
        map: &[draco_core::attributes::geometry_indices::PointIndex],
    ) {
        self.base.set_entry_to_point_id_map(map);
    }

    pub fn clone_with_map(
        &self,
        map: &[draco_core::attributes::geometry_indices::PointIndex],
    ) -> Self {
        // MeshPredictionSchemeGeometricNormalPredictorBase is pointer-only state (no ownership),
        // so a bitwise copy is safe and matches Copy semantics.
        let mut cloned = unsafe { std::ptr::read(&self.base) };
        cloned.set_entry_to_point_id_map(map);
        Self { base: cloned }
    }

    pub fn is_initialized(&self) -> bool {
        self.base.is_initialized()
    }

    pub fn set_normal_prediction_mode(&mut self, mode: NormalPredictionMode) -> bool {
        if mode == NormalPredictionMode::OneTriangle || mode == NormalPredictionMode::TriangleArea {
            self.base.set_normal_prediction_mode(mode);
            true
        } else {
            false
        }
    }

    pub fn normal_prediction_mode(&self) -> NormalPredictionMode {
        self.base.normal_prediction_mode()
    }

    pub fn compute_predicted_value(&self, corner_id: CornerIndex, prediction: &mut [DataTypeT]) {
        let corner_table = self.base.mesh_data().corner_table();
        let mut cit = VertexCornersIterator::from_corner(corner_table, corner_id);
        let pos_cent = self.base.get_position_for_corner(corner_id);

        let mut normal = VectorD::<i64, 3>::new3(0, 0, 0);
        while !cit.end() {
            let (c_next, c_prev) =
                if self.base.normal_prediction_mode() == NormalPredictionMode::OneTriangle {
                    (
                        corner_table.next(corner_id),
                        corner_table.previous(corner_id),
                    )
                } else {
                    let c = cit.corner();
                    (corner_table.next(c), corner_table.previous(c))
                };

            let pos_next = self.base.get_position_for_corner(c_next);
            let pos_prev = self.base.get_position_for_corner(c_prev);
            let delta_next = pos_next - pos_cent;
            let delta_prev = pos_prev - pos_cent;
            let cross = cross_product(&delta_next, &delta_prev);

            unsafe {
                let normal_data = std::slice::from_raw_parts_mut(normal.data_mut() as *mut u64, 3);
                let cross_data = std::slice::from_raw_parts(cross.data() as *const u64, 3);
                for i in 0..3 {
                    normal_data[i] = normal_data[i].wrapping_add(cross_data[i]);
                }
            }

            cit.next();
        }

        let upper_bound: i64 = 1 << 29;
        if self.base.normal_prediction_mode() == NormalPredictionMode::OneTriangle {
            let abs_sum_i32 = normal.abs_sum() as i32;
            if (abs_sum_i32 as i64) > upper_bound {
                let quotient = (abs_sum_i32 as i64) / upper_bound;
                if quotient != 0 {
                    normal = normal / quotient;
                }
            }
        } else {
            let abs_sum = normal.abs_sum();
            if abs_sum > upper_bound {
                let quotient = abs_sum / upper_bound;
                if quotient != 0 {
                    normal = normal / quotient;
                }
            }
        }

        prediction[0] = NumCast::from(normal[0] as i32).unwrap_or_default();
        prediction[1] = NumCast::from(normal[1] as i32).unwrap_or_default();
        prediction[2] = NumCast::from(normal[2] as i32).unwrap_or_default();
    }
}
