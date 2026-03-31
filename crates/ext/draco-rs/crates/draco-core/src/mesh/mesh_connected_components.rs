//! Mesh connected components utilities.
//! Reference: `_ref/draco/src/draco/mesh/mesh_connected_components.h`.

use crate::attributes::geometry_indices::{CornerIndex, FaceIndex, INVALID_CORNER_INDEX};
use crate::mesh::corner_table::CornerTable;
use crate::mesh::corner_table_iterators::CornerTableTraversal;
use crate::mesh::mesh_attribute_corner_table::MeshAttributeCornerTable;

pub trait ConnectedComponentsTable: CornerTableTraversal {
    fn num_vertices(&self) -> usize;
    fn num_faces(&self) -> usize;
    fn num_corners(&self) -> usize;
    fn is_degenerated(&self, face: FaceIndex) -> bool;
    fn all_corners(&self, face: FaceIndex) -> [CornerIndex; 3];
}

impl ConnectedComponentsTable for CornerTable {
    fn num_vertices(&self) -> usize {
        CornerTable::num_vertices(self)
    }
    fn num_faces(&self) -> usize {
        CornerTable::num_faces(self)
    }
    fn num_corners(&self) -> usize {
        CornerTable::num_corners(self)
    }
    fn is_degenerated(&self, face: FaceIndex) -> bool {
        CornerTable::is_degenerated(self, face)
    }
    fn all_corners(&self, face: FaceIndex) -> [CornerIndex; 3] {
        CornerTable::all_corners(self, face)
    }
}

impl<'a> ConnectedComponentsTable for MeshAttributeCornerTable<'a> {
    fn num_vertices(&self) -> usize {
        MeshAttributeCornerTable::num_vertices(self)
    }
    fn num_faces(&self) -> usize {
        MeshAttributeCornerTable::num_faces(self)
    }
    fn num_corners(&self) -> usize {
        MeshAttributeCornerTable::num_corners(self)
    }
    fn is_degenerated(&self, face: FaceIndex) -> bool {
        MeshAttributeCornerTable::is_degenerated(self, face)
    }
    fn all_corners(&self, face: FaceIndex) -> [CornerIndex; 3] {
        MeshAttributeCornerTable::all_corners(self, face)
    }
}

#[derive(Clone, Debug, Default)]
pub struct ConnectedComponent {
    pub vertices: Vec<i32>,
    pub faces: Vec<i32>,
    pub boundary_edges: Vec<i32>,
}

#[derive(Clone, Debug, Default)]
pub struct MeshConnectedComponents {
    vertex_to_component_map: Vec<i32>,
    face_to_component_map: Vec<i32>,
    boundary_corner_to_component_map: Vec<i32>,
    components: Vec<ConnectedComponent>,
}

impl MeshConnectedComponents {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn find_connected_components<T: ConnectedComponentsTable>(&mut self, corner_table: &T) {
        self.components.clear();
        self.vertex_to_component_map = vec![-1; corner_table.num_vertices()];
        self.face_to_component_map = vec![-1; corner_table.num_faces()];
        self.boundary_corner_to_component_map = vec![-1; corner_table.num_corners()];
        let mut is_face_visited = vec![false; corner_table.num_faces()];
        let mut face_stack: Vec<usize> = Vec::new();

        for face_id in 0..corner_table.num_faces() {
            if is_face_visited[face_id] {
                continue;
            }
            if corner_table.is_degenerated(FaceIndex::from(face_id as u32)) {
                continue;
            }
            let component_id = self.components.len() as i32;
            self.components.push(ConnectedComponent::default());
            face_stack.push(face_id);
            is_face_visited[face_id] = true;
            while let Some(act_face_id) = face_stack.pop() {
                if self.face_to_component_map[act_face_id] == -1 {
                    self.face_to_component_map[act_face_id] = component_id;
                    self.components[component_id as usize]
                        .faces
                        .push(act_face_id as i32);
                }

                let corners = corner_table.all_corners(FaceIndex::from(act_face_id as u32));
                for c in 0..3 {
                    let vertex_id = corner_table.vertex(corners[c]).value() as usize;
                    if self.vertex_to_component_map[vertex_id] == -1 {
                        self.vertex_to_component_map[vertex_id] = component_id;
                        self.components[component_id as usize]
                            .vertices
                            .push(vertex_id as i32);
                    }
                    let opp_corner = corner_table.opposite(corners[c]);
                    if opp_corner == INVALID_CORNER_INDEX {
                        if self.boundary_corner_to_component_map[corners[c].value() as usize] == -1
                        {
                            self.boundary_corner_to_component_map[corners[c].value() as usize] =
                                component_id;
                            self.components[component_id as usize]
                                .boundary_edges
                                .push(corners[c].value() as i32);
                        }
                        continue;
                    }
                    let opp_face_id = corner_table.face(opp_corner).value() as usize;
                    if is_face_visited[opp_face_id] {
                        continue;
                    }
                    is_face_visited[opp_face_id] = true;
                    face_stack.push(opp_face_id);
                }
            }
        }
    }

    pub fn num_connected_components(&self) -> i32 {
        self.components.len() as i32
    }

    pub fn get_connected_component(&self, index: i32) -> &ConnectedComponent {
        &self.components[index as usize]
    }

    pub fn get_connected_component_id_at_vertex(&self, vertex_id: i32) -> i32 {
        if vertex_id < 0 || vertex_id as usize >= self.vertex_to_component_map.len() {
            return -1;
        }
        self.vertex_to_component_map[vertex_id as usize]
    }

    pub fn num_connected_component_vertices(&self, component_id: i32) -> i32 {
        self.components[component_id as usize].vertices.len() as i32
    }

    pub fn get_connected_component_vertex(&self, component_id: i32, i: i32) -> i32 {
        self.components[component_id as usize].vertices[i as usize]
    }

    pub fn get_connected_component_id_at_face(&self, face_id: i32) -> i32 {
        if face_id < 0 || face_id as usize >= self.face_to_component_map.len() {
            return -1;
        }
        self.face_to_component_map[face_id as usize]
    }

    pub fn num_connected_component_faces(&self, component_id: i32) -> i32 {
        self.components[component_id as usize].faces.len() as i32
    }

    pub fn get_connected_component_face(&self, component_id: i32, i: i32) -> i32 {
        self.components[component_id as usize].faces[i as usize]
    }

    pub fn num_connected_component_boundary_edges(&self, component_id: i32) -> i32 {
        self.components[component_id as usize].boundary_edges.len() as i32
    }

    pub fn get_connected_component_boundary_edge(&self, component_id: i32, i: i32) -> i32 {
        self.components[component_id as usize].boundary_edges[i as usize]
    }
}
