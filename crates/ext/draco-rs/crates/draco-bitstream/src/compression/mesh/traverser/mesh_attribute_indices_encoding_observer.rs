//! Mesh attribute indices encoding observer.
//! Reference: `_ref/draco/src/draco/compression/mesh/traverser/mesh_attribute_indices_encoding_observer.h`.

use draco_core::attributes::geometry_indices::{CornerIndex, FaceIndex, PointIndex, VertexIndex};
use draco_core::mesh::mesh::Mesh;

use crate::compression::attributes::mesh_attribute_indices_encoding_data::MeshAttributeIndicesEncodingData;
use crate::compression::mesh::traverser::traverser_base::TraversalObserver;

#[derive(Default)]
pub struct MeshAttributeIndicesEncodingObserver<CornerTableT> {
    // Parity: stored for C++ API symmetry even though unused in current path.
    #[allow(dead_code)]
    att_connectivity: *const CornerTableT,
    encoding_data: *mut MeshAttributeIndicesEncodingData,
    mesh: *const Mesh,
    out_point_ids: *mut Vec<PointIndex>,
}

impl<CornerTableT> MeshAttributeIndicesEncodingObserver<CornerTableT> {
    pub fn new(
        att_connectivity: *const CornerTableT,
        mesh: *const Mesh,
        out_point_ids: *mut Vec<PointIndex>,
        encoding_data: *mut MeshAttributeIndicesEncodingData,
    ) -> Self {
        Self {
            att_connectivity,
            encoding_data,
            mesh,
            out_point_ids,
        }
    }
}

impl<CornerTableT> TraversalObserver for MeshAttributeIndicesEncodingObserver<CornerTableT> {
    fn on_new_face_visited(&mut self, _face: FaceIndex) {}

    fn on_new_vertex_visited(&mut self, vertex: VertexIndex, corner: CornerIndex) {
        unsafe {
            let mesh = &*self.mesh;
            let encoding_data = &mut *self.encoding_data;
            let out_point_ids = &mut *self.out_point_ids;
            let face = mesh.face(FaceIndex::from(corner.value() / 3));
            let point_id = face[(corner.value() % 3) as usize];
            out_point_ids.push(point_id);

            encoding_data
                .encoded_attribute_value_index_to_corner_map
                .push(corner);
            if vertex.value() as usize
                >= encoding_data
                    .vertex_to_encoded_attribute_value_index_map
                    .len()
            {
                encoding_data
                    .vertex_to_encoded_attribute_value_index_map
                    .resize(vertex.value() as usize + 1, -1);
            }
            encoding_data.vertex_to_encoded_attribute_value_index_map[vertex.value() as usize] =
                encoding_data.num_values;
            encoding_data.num_values += 1;
        }
    }
}
