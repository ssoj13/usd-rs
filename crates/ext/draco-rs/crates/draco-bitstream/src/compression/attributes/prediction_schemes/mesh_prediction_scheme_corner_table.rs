//! Corner table trait for mesh prediction schemes.
//! Reference: `_ref/draco/src/draco/compression/attributes/prediction_schemes`.
//!
//! Provides the minimal corner table API required by mesh prediction schemes.

use draco_core::attributes::geometry_indices::{CornerIndex, VertexIndex, INVALID_CORNER_INDEX};
use draco_core::mesh::corner_table::CornerTable;
use draco_core::mesh::mesh_attribute_corner_table::MeshAttributeCornerTable;

pub trait MeshPredictionCornerTable {
    fn next(&self, c: CornerIndex) -> CornerIndex;
    fn previous(&self, c: CornerIndex) -> CornerIndex;
    fn opposite(&self, c: CornerIndex) -> CornerIndex;
    fn vertex(&self, c: CornerIndex) -> VertexIndex;
    fn swing_right(&self, c: CornerIndex) -> CornerIndex;
    fn swing_left(&self, c: CornerIndex) -> CornerIndex;
    fn num_corners(&self) -> usize;
}

impl MeshPredictionCornerTable for CornerTable {
    fn next(&self, c: CornerIndex) -> CornerIndex {
        CornerTable::next(self, c)
    }
    fn previous(&self, c: CornerIndex) -> CornerIndex {
        CornerTable::previous(self, c)
    }
    fn opposite(&self, c: CornerIndex) -> CornerIndex {
        CornerTable::opposite(self, c)
    }
    fn vertex(&self, c: CornerIndex) -> VertexIndex {
        CornerTable::vertex(self, c)
    }
    fn swing_right(&self, c: CornerIndex) -> CornerIndex {
        CornerTable::swing_right(self, c)
    }
    fn swing_left(&self, c: CornerIndex) -> CornerIndex {
        CornerTable::swing_left(self, c)
    }
    fn num_corners(&self) -> usize {
        CornerTable::num_corners(self)
    }
}

impl<'a> MeshPredictionCornerTable for MeshAttributeCornerTable<'a> {
    fn next(&self, c: CornerIndex) -> CornerIndex {
        MeshAttributeCornerTable::next(self, c)
    }
    fn previous(&self, c: CornerIndex) -> CornerIndex {
        MeshAttributeCornerTable::previous(self, c)
    }
    fn opposite(&self, c: CornerIndex) -> CornerIndex {
        MeshAttributeCornerTable::opposite(self, c)
    }
    fn vertex(&self, c: CornerIndex) -> VertexIndex {
        MeshAttributeCornerTable::vertex(self, c)
    }
    fn swing_right(&self, c: CornerIndex) -> CornerIndex {
        MeshAttributeCornerTable::swing_right(self, c)
    }
    fn swing_left(&self, c: CornerIndex) -> CornerIndex {
        MeshAttributeCornerTable::swing_left(self, c)
    }
    fn num_corners(&self) -> usize {
        MeshAttributeCornerTable::num_corners(self)
    }
}

pub fn is_valid_corner(c: CornerIndex) -> bool {
    c != INVALID_CORNER_INDEX
}
