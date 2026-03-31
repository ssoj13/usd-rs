//! Mesh cleanup utilities.
//! Reference: `_ref/draco/src/draco/mesh/mesh_cleanup.h` + `.cc`.

use std::collections::HashSet;

use crate::attributes::geometry_attribute::GeometryAttributeType;
use crate::attributes::geometry_indices::{
    AttributeValueIndex, FaceIndex, PointIndex, INVALID_POINT_INDEX,
};
use crate::core::draco_index_type_vector::IndexTypeVector;
use crate::core::status::{ok_status, Status};
use crate::mesh::mesh::{Face, Mesh};

#[derive(Clone, Debug)]
pub struct MeshCleanupOptions {
    pub remove_degenerated_faces: bool,
    pub remove_duplicate_faces: bool,
    pub remove_unused_attributes: bool,
    pub make_geometry_manifold: bool,
}

impl Default for MeshCleanupOptions {
    fn default() -> Self {
        Self {
            remove_degenerated_faces: true,
            remove_duplicate_faces: true,
            remove_unused_attributes: true,
            make_geometry_manifold: false,
        }
    }
}

pub struct MeshCleanup;

impl MeshCleanup {
    pub fn cleanup(mesh: &mut Mesh, options: &MeshCleanupOptions) -> Status {
        if !options.remove_degenerated_faces
            && !options.remove_unused_attributes
            && !options.remove_duplicate_faces
            && !options.make_geometry_manifold
        {
            return ok_status();
        }

        if mesh
            .get_named_attribute(GeometryAttributeType::Position)
            .is_none()
        {
            return Status::error("Missing position attribute.");
        }

        if options.remove_degenerated_faces {
            Self::remove_degenerated_faces(mesh);
        }
        if options.remove_duplicate_faces {
            Self::remove_duplicate_faces(mesh);
        }
        if options.remove_unused_attributes {
            Self::remove_unused_attributes(mesh);
        }
        if options.make_geometry_manifold {
            let status = Self::make_geometry_manifold(mesh);
            if !status.is_ok() {
                return status;
            }
        }
        ok_status()
    }

    fn remove_degenerated_faces(mesh: &mut Mesh) {
        let mut num_degenerated_faces = 0u32;
        let mut pos_indices = [AttributeValueIndex::from(0u32); 3];
        let num_faces = mesh.num_faces();
        for f in 0..num_faces {
            let fi = FaceIndex::from(f);
            let face = *mesh.face(fi);
            let is_degenerate = {
                let pos_att = mesh
                    .get_named_attribute(GeometryAttributeType::Position)
                    .expect("Position attribute missing");
                for p in 0..3 {
                    pos_indices[p] = pos_att.mapped_index(face[p]);
                }
                pos_indices[0] == pos_indices[1]
                    || pos_indices[0] == pos_indices[2]
                    || pos_indices[1] == pos_indices[2]
            };
            if is_degenerate {
                num_degenerated_faces += 1;
            } else if num_degenerated_faces > 0 {
                let new_fi = FaceIndex::from(f - num_degenerated_faces);
                mesh.set_face(new_fi, face);
            }
        }
        if num_degenerated_faces > 0 {
            mesh.set_num_faces((mesh.num_faces() - num_degenerated_faces) as usize);
        }
    }

    fn remove_duplicate_faces(mesh: &mut Mesh) {
        let mut is_face_used: HashSet<Face> = HashSet::new();
        let mut num_duplicate_faces = 0u32;
        let num_faces = mesh.num_faces();
        for f in 0..num_faces {
            let fi = FaceIndex::from(f);
            let mut face = *mesh.face(fi);
            while face[0] > face[1] || face[0] > face[2] {
                face = rotate_face_left(face);
            }
            if is_face_used.contains(&face) {
                num_duplicate_faces += 1;
            } else {
                is_face_used.insert(face);
                if num_duplicate_faces > 0 {
                    let new_fi = FaceIndex::from(f - num_duplicate_faces);
                    mesh.set_face(new_fi, face);
                }
            }
        }
        if num_duplicate_faces > 0 {
            mesh.set_num_faces((mesh.num_faces() - num_duplicate_faces) as usize);
        }
    }

    fn remove_unused_attributes(mesh: &mut Mesh) {
        let mut is_point_used = vec![false; mesh.num_points() as usize];
        let mut num_new_points = 0u32;
        let num_faces = mesh.num_faces();
        for f in 0..num_faces {
            let fi = FaceIndex::from(f);
            let face = mesh.face(fi);
            for p in 0..3 {
                let idx = face[p].value() as usize;
                if !is_point_used[idx] {
                    is_point_used[idx] = true;
                    num_new_points += 1;
                }
            }
        }

        let num_original_points = mesh.num_points();
        let mut point_map = IndexTypeVector::<PointIndex, PointIndex>::with_size_value(
            num_original_points as usize,
            INVALID_POINT_INDEX,
        );
        let mut points_changed = false;

        if num_new_points < num_original_points {
            num_new_points = 0;
            for i in 0..num_original_points {
                let pi = PointIndex::from(i);
                if is_point_used[i as usize] {
                    point_map[pi] = PointIndex::from(num_new_points);
                    num_new_points += 1;
                } else {
                    point_map[pi] = INVALID_POINT_INDEX;
                }
            }
            for f in 0..mesh.num_faces() {
                let fi = FaceIndex::from(f);
                let mut face = *mesh.face(fi);
                for p in 0..3 {
                    face[p] = point_map[face[p]];
                }
                mesh.set_face(fi, face);
            }
            mesh.set_num_points(num_new_points);
            points_changed = true;
        } else {
            for i in 0..num_original_points {
                let pi = PointIndex::from(i);
                point_map[pi] = pi;
            }
        }

        let current_num_points = mesh.num_points();
        let mut is_att_index_used: IndexTypeVector<AttributeValueIndex, u8> =
            IndexTypeVector::new();
        let mut att_index_map: IndexTypeVector<AttributeValueIndex, AttributeValueIndex> =
            IndexTypeVector::new();
        for a in 0..mesh.num_attributes() {
            let att = match mesh.attribute_mut(a) {
                Some(att) => att,
                None => continue,
            };
            is_att_index_used.assign(att.size(), 0);
            att_index_map.clear();
            let mut num_used_entries = 0u32;
            for i in 0..num_original_points {
                let pi = PointIndex::from(i);
                if point_map[pi] != INVALID_POINT_INDEX {
                    let entry_id = att.mapped_index(pi);
                    if is_att_index_used[entry_id] == 0 {
                        is_att_index_used[entry_id] = 1;
                        num_used_entries += 1;
                    }
                }
            }
            let mut att_indices_changed = false;
            if num_used_entries < att.size() as u32 {
                att_index_map.resize(att.size());
                num_used_entries = 0;
                let stride = att.byte_stride() as usize;
                let mut tmp = vec![0u8; stride];
                for i in 0..att.size() {
                    let avi = AttributeValueIndex::from(i as u32);
                    if is_att_index_used[avi] != 0 {
                        att_index_map[avi] = AttributeValueIndex::from(num_used_entries);
                        if i as u32 > num_used_entries {
                            att.get_value_bytes(avi, &mut tmp);
                            att.set_attribute_value_bytes(
                                AttributeValueIndex::from(num_used_entries),
                                &tmp,
                            );
                        }
                        num_used_entries += 1;
                    }
                }
                att.resize(num_used_entries as usize);
                att_indices_changed = true;
            }

            if points_changed || att_indices_changed {
                if att.is_mapping_identity() {
                    if num_used_entries != current_num_points {
                        att.set_explicit_mapping(num_original_points as usize);
                        for i in 0..num_original_points {
                            att.set_point_map_entry(
                                PointIndex::from(i),
                                AttributeValueIndex::from(i),
                            );
                        }
                    }
                }
                if !att.is_mapping_identity() {
                    for i in 0..num_original_points {
                        let pi = PointIndex::from(i);
                        let new_point_id = point_map[pi];
                        if new_point_id == INVALID_POINT_INDEX {
                            continue;
                        }
                        let original_entry_index = att.mapped_index(pi);
                        let new_entry_index = if att_indices_changed {
                            att_index_map[original_entry_index]
                        } else {
                            original_entry_index
                        };
                        att.set_point_map_entry(new_point_id, new_entry_index);
                    }
                    att.set_explicit_mapping(current_num_points as usize);
                }
            }
        }
    }

    fn make_geometry_manifold(_mesh: &mut Mesh) -> Status {
        // Parity: C++ reference reports this as unsupported.
        Status::error("Unsupported function.")
    }
}

fn rotate_face_left(face: Face) -> Face {
    [face[1], face[2], face[0]]
}
