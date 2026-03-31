//! Mesh equivalence checker (transcoder-related).
//!
//! What: Compares two meshes for equivalence up to vertex permutation.
//! Why: Mirrors Draco `MeshAreEquivalent` used in tests and validation.
//! How: Orders faces lexicographically by positions and compares per-corner data.
//! Where used: Transcoder parity checks and mesh validation.

use std::cmp::Ordering;

use crate::attributes::geometry_attribute::GeometryAttributeType;
use crate::attributes::geometry_indices::FaceIndex;
use crate::core::draco_index_type_vector::IndexTypeVector;
use crate::core::vector_d::Vector3f;
use crate::mesh::mesh::Mesh;
use crate::mesh::mesh_indices::MeshFeaturesIndex;

/// Compares two meshes for equivalence up to vertex permutation.
#[derive(Default)]
pub struct MeshAreEquivalent;

impl MeshAreEquivalent {
    pub fn new() -> Self {
        Self
    }

    /// Returns true if both meshes are equivalent up to permutation of vertices.
    pub fn are_equivalent(&self, mesh0: &Mesh, mesh1: &Mesh) -> bool {
        if mesh0.num_faces() != mesh1.num_faces() {
            return false;
        }
        if mesh0.num_attributes() != mesh1.num_attributes() {
            return false;
        }

        let num_faces = mesh0.num_faces() as i32;
        let mut mesh_infos = vec![MeshInfo::new(mesh0), MeshInfo::new(mesh1)];
        init_corner_index_of_smallest_point_xyz(&mut mesh_infos, num_faces);
        init_ordered_face_index(&mut mesh_infos, num_faces);

        if mesh0.is_compression_enabled() != mesh1.is_compression_enabled() {
            return false;
        }
        if mesh0.compression_options() != mesh1.compression_options() {
            return false;
        }

        if mesh0.non_material_texture_library().num_textures()
            != mesh1.non_material_texture_library().num_textures()
        {
            return false;
        }

        if mesh0.num_mesh_features() != mesh1.num_mesh_features() {
            return false;
        }
        for i in 0..mesh0.num_mesh_features() {
            let index = MeshFeaturesIndex::from(i as u32);
            let features0 = mesh0.mesh_features(index);
            let features1 = mesh1.mesh_features(index);
            if features0.attribute_index() != features1.attribute_index() {
                return false;
            }
            if features0.feature_count() != features1.feature_count() {
                return false;
            }
            if features0.label() != features1.label() {
                return false;
            }
            if features0.null_feature_id() != features1.null_feature_id() {
                return false;
            }
            if features0.texture_channels() != features1.texture_channels() {
                return false;
            }
            if features0.property_table_index() != features1.property_table_index() {
                return false;
            }
            if features0.texture_map().tex_coord_index()
                != features1.texture_map().tex_coord_index()
            {
                return false;
            }
        }

        // Dynamic iteration matching C++ `for (att_id = 0; att_id < NAMED_ATTRIBUTES_COUNT; ++att_id)`.
        for att_type in GeometryAttributeType::ALL_NAMED {
            let att0 = mesh0.get_named_attribute(att_type);
            let att1 = mesh1.get_named_attribute(att_type);
            if att0.is_none() && att1.is_none() {
                continue;
            }
            let att0 = match att0 {
                Some(att) => att,
                None => return false,
            };
            let att1 = match att1 {
                Some(att) => att,
                None => return false,
            };

            if att0.data_type() != att1.data_type() {
                return false;
            }
            if att0.num_components() != att1.num_components() {
                return false;
            }
            if att0.normalized() != att1.normalized() {
                return false;
            }
            if att0.byte_stride() != att1.byte_stride() {
                return false;
            }

            let stride = att0.byte_stride() as usize;
            let mut data0 = vec![0u8; stride];
            let mut data1 = vec![0u8; stride];

            for i in 0..num_faces {
                let f0 = mesh_infos[0].ordered_index_of_face[i as usize];
                let f1 = mesh_infos[1].ordered_index_of_face[i as usize];
                let c0_off = mesh_infos[0].corner_index_of_smallest_vertex[f0];
                let c1_off = mesh_infos[1].corner_index_of_smallest_vertex[f1];

                for c in 0..3 {
                    let corner0 = mesh0.face(f0)[((c0_off + c) % 3) as usize];
                    let corner1 = mesh1.face(f1)[((c1_off + c) % 3) as usize];
                    let index0 = att0.mapped_index(corner0);
                    let index1 = att1.mapped_index(corner1);
                    att0.get_value_bytes(index0, &mut data0);
                    att1.get_value_bytes(index1, &mut data1);
                    if data0 != data1 {
                        return false;
                    }
                }
            }
        }
        true
    }
}

struct MeshInfo<'a> {
    mesh: &'a Mesh,
    ordered_index_of_face: Vec<FaceIndex>,
    corner_index_of_smallest_vertex: IndexTypeVector<FaceIndex, i32>,
}

impl<'a> MeshInfo<'a> {
    fn new(mesh: &'a Mesh) -> Self {
        Self {
            mesh,
            ordered_index_of_face: Vec::new(),
            corner_index_of_smallest_vertex: IndexTypeVector::new(),
        }
    }
}

fn get_position(mesh: &Mesh, f: FaceIndex, c: i32) -> Vector3f {
    let pos_att = mesh
        .get_named_attribute(GeometryAttributeType::Position)
        .expect("Missing position attribute");
    let vertex_index = mesh.face(f)[c as usize];
    let pos_index = pos_att.mapped_index(vertex_index);
    let mut pos = [0.0f32; 3];
    pos_att.get_value_array_into(pos_index, &mut pos);
    Vector3f::new3(pos[0], pos[1], pos[2])
}

fn compute_corner_index_of_smallest_point_xyz(mesh: &Mesh, f: FaceIndex) -> i32 {
    let mut min_index = 0;
    let mut min_pos = get_position(mesh, f, 0);
    for i in 1..3 {
        let pos = get_position(mesh, f, i);
        if pos < min_pos {
            min_pos = pos;
            min_index = i;
        }
    }
    min_index
}

fn init_corner_index_of_smallest_point_xyz(mesh_infos: &mut [MeshInfo<'_>], num_faces: i32) {
    for mesh_info in mesh_infos.iter_mut() {
        mesh_info
            .corner_index_of_smallest_vertex
            .reserve(num_faces as usize);
        for f in 0..num_faces {
            let index = compute_corner_index_of_smallest_point_xyz(
                mesh_info.mesh,
                FaceIndex::from(f as u32),
            );
            mesh_info.corner_index_of_smallest_vertex.push_back(index);
        }
    }
}

fn face_index_less(mesh_info: &MeshInfo<'_>, f0: FaceIndex, f1: FaceIndex) -> bool {
    if f0 == f1 {
        return false;
    }
    let c0 = mesh_info.corner_index_of_smallest_vertex[f0];
    let c1 = mesh_info.corner_index_of_smallest_vertex[f1];

    for i in 0..3 {
        let vf0 = get_position(mesh_info.mesh, f0, (c0 + i) % 3);
        let vf1 = get_position(mesh_info.mesh, f1, (c1 + i) % 3);
        if vf0 < vf1 {
            return true;
        }
        if vf1 < vf0 {
            return false;
        }
    }
    false
}

fn init_ordered_face_index(mesh_infos: &mut [MeshInfo<'_>], num_faces: i32) {
    for mesh_info in mesh_infos.iter_mut() {
        let mut ordered: Vec<FaceIndex> =
            (0..num_faces).map(|f| FaceIndex::from(f as u32)).collect();
        {
            let mesh_ref = &*mesh_info;
            ordered.sort_by(|a, b| {
                if face_index_less(mesh_ref, *a, *b) {
                    Ordering::Less
                } else if face_index_less(mesh_ref, *b, *a) {
                    Ordering::Greater
                } else {
                    Ordering::Equal
                }
            });
        }
        mesh_info.ordered_index_of_face = ordered;
    }
}
