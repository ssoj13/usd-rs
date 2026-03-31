//! Shared helpers for parallelogram-based mesh prediction.
//! Reference: `_ref/draco/src/draco/compression/attributes/prediction_schemes/mesh_prediction_scheme_parallelogram_shared.h`.

use draco_core::attributes::geometry_indices::{CornerIndex, INVALID_CORNER_INDEX};
use num_traits::{cast, NumCast};

use crate::compression::attributes::prediction_schemes::mesh_prediction_scheme_corner_table::MeshPredictionCornerTable;

pub fn get_parallelogram_entries<CornerTableT: MeshPredictionCornerTable>(
    ci: CornerIndex,
    table: &CornerTableT,
    vertex_to_data_map: &[i32],
    opp_entry: &mut i32,
    next_entry: &mut i32,
    prev_entry: &mut i32,
) {
    *opp_entry = vertex_to_data_map[table.vertex(ci).value() as usize];
    *next_entry = vertex_to_data_map[table.vertex(table.next(ci)).value() as usize];
    *prev_entry = vertex_to_data_map[table.vertex(table.previous(ci)).value() as usize];
}

pub fn compute_parallelogram_prediction<CornerTableT, DataTypeT>(
    data_entry_id: i32,
    ci: CornerIndex,
    table: &CornerTableT,
    vertex_to_data_map: &[i32],
    in_data: &[DataTypeT],
    num_components: i32,
    out_prediction: &mut [DataTypeT],
) -> bool
where
    CornerTableT: MeshPredictionCornerTable,
    DataTypeT: Copy + Default + NumCast,
{
    let oci = table.opposite(ci);
    if oci == INVALID_CORNER_INDEX {
        return false;
    }
    let mut vert_opp = 0i32;
    let mut vert_next = 0i32;
    let mut vert_prev = 0i32;
    get_parallelogram_entries(
        oci,
        table,
        vertex_to_data_map,
        &mut vert_opp,
        &mut vert_next,
        &mut vert_prev,
    );
    if vert_opp >= 0
        && vert_next >= 0
        && vert_prev >= 0
        && vert_opp < data_entry_id
        && vert_next < data_entry_id
        && vert_prev < data_entry_id
    {
        let num_components_usize = num_components as usize;
        let v_opp_off = (vert_opp as usize) * num_components_usize;
        let v_next_off = (vert_next as usize) * num_components_usize;
        let v_prev_off = (vert_prev as usize) * num_components_usize;
        for c in 0..num_components_usize {
            let in_next: i64 = cast(in_data[v_next_off + c]).unwrap_or_default();
            let in_prev: i64 = cast(in_data[v_prev_off + c]).unwrap_or_default();
            let in_opp: i64 = cast(in_data[v_opp_off + c]).unwrap_or_default();
            let result: i64 = (in_next + in_prev) - in_opp;
            out_prediction[c] = cast(result).unwrap_or_default();
        }
        return true;
    }
    false
}
