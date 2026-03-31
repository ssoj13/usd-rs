//! Mesh attribute indices encoding data.
//! Reference: `_ref/draco/src/draco/compression/attributes/mesh_attribute_indices_encoding_data.h`.

use draco_core::attributes::geometry_indices::CornerIndex;

/// Data used for encoding and decoding mesh attribute indices.
#[derive(Clone, Debug, Default)]
pub struct MeshAttributeIndicesEncodingData {
    /// Mapping from encoded attribute value index to a representative corner.
    pub encoded_attribute_value_index_to_corner_map: Vec<CornerIndex>,
    /// Mapping from vertex id to encoded attribute value index.
    /// Value is -1 when the entry has not been encoded yet.
    pub vertex_to_encoded_attribute_value_index_map: Vec<i32>,
    /// Total number of encoded/decoded attribute entries.
    pub num_values: i32,
}

impl MeshAttributeIndicesEncodingData {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn init(&mut self, num_vertices: usize) {
        self.vertex_to_encoded_attribute_value_index_map
            .resize(num_vertices, -1);
        self.encoded_attribute_value_index_to_corner_map
            .reserve(num_vertices);
        self.num_values = 0;
    }

    pub fn assign_vertex_to_encoded_map(&mut self, num_vertices: usize, value: i32) {
        self.vertex_to_encoded_attribute_value_index_map.clear();
        self.vertex_to_encoded_attribute_value_index_map
            .resize(num_vertices, value);
    }
}
