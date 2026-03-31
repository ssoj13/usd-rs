//! Mesh prediction scheme data holder.
//! Reference: `_ref/draco/src/draco/compression/attributes/prediction_schemes/mesh_prediction_scheme_data.h`.
//!
//! Stores mesh connectivity pointers and attribute encoding maps needed by mesh
//! prediction schemes during decoding.

use draco_core::attributes::geometry_indices::CornerIndex;
use draco_core::mesh::mesh::Mesh;

pub trait MeshPredictionSchemeDataRef {
    type CornerTable;
    fn mesh(&self) -> &Mesh;
    fn corner_table(&self) -> &Self::CornerTable;
    fn vertex_to_data_map(&self) -> &Vec<i32>;
    fn data_to_corner_map(&self) -> &Vec<CornerIndex>;
    fn is_initialized(&self) -> bool;
}

pub struct MeshPredictionSchemeData<CornerTableT> {
    mesh: *const Mesh,
    corner_table: *const CornerTableT,
    vertex_to_data_map: *const Vec<i32>,
    data_to_corner_map: *const Vec<CornerIndex>,
}

impl<CornerTableT> MeshPredictionSchemeData<CornerTableT> {
    pub fn new() -> Self {
        Self {
            mesh: std::ptr::null(),
            corner_table: std::ptr::null(),
            vertex_to_data_map: std::ptr::null(),
            data_to_corner_map: std::ptr::null(),
        }
    }

    pub fn set(
        &mut self,
        mesh: &Mesh,
        corner_table: &CornerTableT,
        data_to_corner_map: &Vec<CornerIndex>,
        vertex_to_data_map: &Vec<i32>,
    ) {
        self.mesh = mesh as *const Mesh;
        self.corner_table = corner_table as *const CornerTableT;
        self.data_to_corner_map = data_to_corner_map as *const Vec<CornerIndex>;
        self.vertex_to_data_map = vertex_to_data_map as *const Vec<i32>;
    }

    pub fn mesh(&self) -> &Mesh {
        unsafe { &*self.mesh }
    }

    pub fn corner_table(&self) -> &CornerTableT {
        unsafe { &*self.corner_table }
    }

    pub fn vertex_to_data_map(&self) -> &Vec<i32> {
        unsafe { &*self.vertex_to_data_map }
    }

    pub fn data_to_corner_map(&self) -> &Vec<CornerIndex> {
        unsafe { &*self.data_to_corner_map }
    }

    pub fn is_initialized(&self) -> bool {
        !self.mesh.is_null()
            && !self.corner_table.is_null()
            && !self.vertex_to_data_map.is_null()
            && !self.data_to_corner_map.is_null()
    }
}

impl<CornerTableT> MeshPredictionSchemeDataRef for MeshPredictionSchemeData<CornerTableT> {
    type CornerTable = CornerTableT;

    fn mesh(&self) -> &Mesh {
        MeshPredictionSchemeData::mesh(self)
    }

    fn corner_table(&self) -> &Self::CornerTable {
        MeshPredictionSchemeData::corner_table(self)
    }

    fn vertex_to_data_map(&self) -> &Vec<i32> {
        MeshPredictionSchemeData::vertex_to_data_map(self)
    }

    fn data_to_corner_map(&self) -> &Vec<CornerIndex> {
        MeshPredictionSchemeData::data_to_corner_map(self)
    }

    fn is_initialized(&self) -> bool {
        MeshPredictionSchemeData::is_initialized(self)
    }
}

impl<CornerTableT> Copy for MeshPredictionSchemeData<CornerTableT> {}

impl<CornerTableT> Clone for MeshPredictionSchemeData<CornerTableT> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<CornerTableT> Default for MeshPredictionSchemeData<CornerTableT> {
    fn default() -> Self {
        Self::new()
    }
}
