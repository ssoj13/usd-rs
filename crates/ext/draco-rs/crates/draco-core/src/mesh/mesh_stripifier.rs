//! Mesh stripifier.
//! Reference: `_ref/draco/src/draco/mesh/mesh_stripifier.h` + `.cc`.

use crate::attributes::geometry_indices::{
    CornerIndex, FaceIndex, PointIndex, INVALID_CORNER_INDEX, INVALID_POINT_INDEX,
};
use crate::core::draco_index_type_vector::IndexTypeVector;
use crate::mesh::corner_table::CornerTable;
use crate::mesh::mesh::Mesh;
use crate::mesh::mesh_misc_functions::create_corner_table_from_position_attribute;

pub struct MeshStripifier {
    mesh: Option<*const Mesh>,
    corner_table: Option<CornerTable>,
    strip_faces: [Vec<FaceIndex>; 3],
    strip_start_corners: [CornerIndex; 3],
    is_face_visited: IndexTypeVector<FaceIndex, bool>,
    num_strips: i32,
    num_encoded_faces: i32,
    last_encoded_point: PointIndex,
}

impl MeshStripifier {
    pub fn new() -> Self {
        Self {
            mesh: None,
            corner_table: None,
            strip_faces: std::array::from_fn(|_| Vec::new()),
            strip_start_corners: [INVALID_CORNER_INDEX; 3],
            is_face_visited: IndexTypeVector::new(),
            num_strips: 0,
            num_encoded_faces: 0,
            last_encoded_point: INVALID_POINT_INDEX,
        }
    }

    pub fn num_strips(&self) -> i32 {
        self.num_strips
    }

    pub fn generate_triangle_strips_with_primitive_restart<IndexTypeT: Copy + From<u32>>(
        &mut self,
        mesh: &Mesh,
        primitive_restart_index: IndexTypeT,
        out: &mut Vec<IndexTypeT>,
    ) -> bool {
        if !self.prepare(mesh) {
            return false;
        }
        let num_faces = mesh.num_faces();
        for fi in 0..num_faces {
            let face_index = FaceIndex::from(fi);
            if self.is_face_visited[face_index] {
                continue;
            }
            let longest_strip_id = self.find_longest_strip_from_face(face_index);
            if self.num_strips > 0 {
                out.push(primitive_restart_index);
            }
            self.store_strip(longest_strip_id, out);
        }
        true
    }

    pub fn generate_triangle_strips_with_degenerate_triangles<IndexTypeT: Copy + From<u32>>(
        &mut self,
        mesh: &Mesh,
        out: &mut Vec<IndexTypeT>,
    ) -> bool {
        if !self.prepare(mesh) {
            return false;
        }
        let num_faces = mesh.num_faces();
        for fi in 0..num_faces {
            let face_index = FaceIndex::from(fi);
            if self.is_face_visited[face_index] {
                continue;
            }
            let longest_strip_id = self.find_longest_strip_from_face(face_index);
            if self.num_strips > 0 {
                out.push(IndexTypeT::from(self.last_encoded_point.value()));
                let new_start_corner = self.strip_start_corners[longest_strip_id as usize];
                let new_start_point = self.corner_to_point_index(new_start_corner);
                out.push(IndexTypeT::from(new_start_point.value()));
                self.num_encoded_faces += 2;
                if (self.num_encoded_faces & 1) != 0 {
                    out.push(IndexTypeT::from(new_start_point.value()));
                    self.num_encoded_faces += 1;
                }
            }
            self.store_strip(longest_strip_id, out);
        }
        true
    }

    fn prepare(&mut self, mesh: &Mesh) -> bool {
        self.mesh = Some(mesh as *const Mesh);
        self.num_strips = 0;
        self.num_encoded_faces = 0;
        self.corner_table = create_corner_table_from_position_attribute(mesh);
        if self.corner_table.is_none() {
            return false;
        }
        self.is_face_visited
            .assign(mesh.num_faces() as usize, false);
        true
    }

    fn find_longest_strip_from_face(&mut self, fi: FaceIndex) -> i32 {
        let first_ci = self.corner_table().first_corner(fi);
        let mut longest_strip_id = -1;
        let mut longest_strip_length = 0usize;
        for i in 0..3 {
            self.generate_strips_from_corner(i, CornerIndex::from(first_ci.value() + i as u32));
            if self.strip_faces[i].len() > longest_strip_length {
                longest_strip_length = self.strip_faces[i].len();
                longest_strip_id = i as i32;
            }
        }
        longest_strip_id
    }

    fn store_strip<IndexTypeT: Copy + From<u32>>(
        &mut self,
        local_strip_id: i32,
        out: &mut Vec<IndexTypeT>,
    ) {
        self.num_strips += 1;
        let strip_id = local_strip_id as usize;
        let num_strip_faces = self.strip_faces[strip_id].len();
        let mut ci = self.strip_start_corners[strip_id];
        for i in 0..num_strip_faces {
            let fi = self.corner_table().face(ci);
            self.is_face_visited[fi] = true;
            self.num_encoded_faces += 1;

            if i == 0 {
                out.push(IndexTypeT::from(self.corner_to_point_index(ci).value()));
                out.push(IndexTypeT::from(
                    self.corner_to_point_index(self.corner_table().next(ci))
                        .value(),
                ));
                self.last_encoded_point =
                    self.corner_to_point_index(self.corner_table().previous(ci));
                out.push(IndexTypeT::from(self.last_encoded_point.value()));
            } else {
                self.last_encoded_point = self.corner_to_point_index(ci);
                out.push(IndexTypeT::from(self.last_encoded_point.value()));
                if (i & 1) == 1 {
                    ci = self.corner_table().previous(ci);
                } else {
                    ci = self.corner_table().next(ci);
                }
            }
            ci = self.corner_table().opposite(ci);
        }
    }

    fn corner_to_point_index(&self, ci: CornerIndex) -> PointIndex {
        unsafe { &*self.mesh.expect("Mesh missing") }.corner_to_point_id(ci)
    }

    fn get_opposite_corner(&self, ci: CornerIndex) -> CornerIndex {
        let oci = self.corner_table().opposite(ci);
        if oci == INVALID_CORNER_INDEX {
            return INVALID_CORNER_INDEX;
        }
        if self.corner_to_point_index(self.corner_table().next(ci))
            != self.corner_to_point_index(self.corner_table().previous(oci))
        {
            return INVALID_CORNER_INDEX;
        }
        if self.corner_to_point_index(self.corner_table().previous(ci))
            != self.corner_to_point_index(self.corner_table().next(oci))
        {
            return INVALID_CORNER_INDEX;
        }
        oci
    }

    fn generate_strips_from_corner(&mut self, local_strip_id: usize, mut ci: CornerIndex) {
        self.strip_faces[local_strip_id].clear();
        let mut start_ci = ci;
        let mut fi = self.corner_table().face(ci);
        for pass in 0..2 {
            if pass == 1 {
                if self.get_opposite_corner(self.corner_table().previous(start_ci))
                    == INVALID_CORNER_INDEX
                {
                    break;
                }
                ci = self.corner_table().next(start_ci);
                ci = self.corner_table().swing_left(ci);
                if ci == INVALID_CORNER_INDEX {
                    break;
                }
                fi = self.corner_table().face(ci);
            }
            let mut num_added_faces = 0;
            while !self.is_face_visited[fi] {
                self.is_face_visited[fi] = true;
                self.strip_faces[local_strip_id].push(fi);
                num_added_faces += 1;
                if num_added_faces > 1 {
                    if (num_added_faces & 1) == 1 {
                        ci = self.corner_table().next(ci);
                    } else {
                        if pass == 1 {
                            start_ci = ci;
                        }
                        ci = self.corner_table().previous(ci);
                    }
                }
                ci = self.get_opposite_corner(ci);
                if ci == INVALID_CORNER_INDEX {
                    break;
                }
                fi = self.corner_table().face(ci);
            }
            if pass == 1 && (num_added_faces & 1) == 1 {
                if let Some(last) = self.strip_faces[local_strip_id].pop() {
                    self.is_face_visited[last] = false;
                }
            }
        }
        self.strip_start_corners[local_strip_id] = start_ci;
        for face in &self.strip_faces[local_strip_id] {
            self.is_face_visited[*face] = false;
        }
    }

    fn corner_table(&self) -> &CornerTable {
        self.corner_table.as_ref().expect("Corner table missing")
    }
}

impl Default for MeshStripifier {
    fn default() -> Self {
        Self::new()
    }
}
