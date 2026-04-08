use super::error::{ErrorType, far_error};
use super::topology_refiner::TopologyRefiner;
use super::types::Index;
use crate::sdc::types::SchemeTypeTraits;

/// Maps coarse face indices to Ptex face indices.
///
/// Mirrors C++ `Far::PtexIndices`.
///
/// The internal array has `nfaces + 1` entries: `ptex_indices[i]` is the
/// first Ptex face of coarse face `i`, and `ptex_indices[nfaces]` holds
/// the total number of Ptex faces (same as C++ reference).
pub struct PtexIndices {
    /// Size = nfaces + 1; last entry = total Ptex face count.
    ptex_indices: Vec<Index>,
    /// Regular face size (4 for Catmark/Bilinear, 3 for Loop).
    reg_face_size: i32,
}

impl PtexIndices {
    /// Compute Ptex indices from a refiner.
    ///
    /// Mirrors C++ `PtexIndices::PtexIndices(TopologyRefiner const&)`.
    pub fn new(refiner: &TopologyRefiner) -> Self {
        let reg_face_size = SchemeTypeTraits::get_regular_face_size(refiner.get_scheme_type());

        let lv = refiner.get_level(0);
        let nf = lv.get_num_faces() as usize;

        // Allocate nfaces+1 entries — last entry holds total count
        let mut ptex_indices = vec![0i32; nf + 1];
        let mut ptex_id = 0i32;
        for i in 0..nf {
            ptex_indices[i] = ptex_id;
            let nv = lv.get_face_vertices(i as Index).size();
            // Regular face → 1 Ptex face; n-gon → n sub-quads
            ptex_id += if nv == reg_face_size { 1 } else { nv };
        }
        ptex_indices[nf] = ptex_id;

        Self {
            ptex_indices,
            reg_face_size,
        }
    }

    /// Total number of Ptex faces in the mesh.
    ///
    /// Mirrors C++ `PtexIndices::GetNumFaces()` which returns `_ptexIndices.back()`.
    #[doc(alias = "GetNumFaces")]
    pub fn get_num_faces(&self) -> i32 {
        // Last entry stores the total count
        self.ptex_indices.last().copied().unwrap_or(0)
    }

    /// Ptex face index for base face `f`.
    ///
    /// Mirrors C++ `PtexIndices::GetFaceId(Index f)`.
    #[doc(alias = "GetFaceId")]
    pub fn get_face_id(&self, f: Index) -> Index {
        self.ptex_indices[f as usize]
    }

    /// Fill adjacency information for coarse face `face` / quadrant `quadrant`.
    ///
    /// `adj_faces[4]` receives ptex face indices of adjacent faces;
    /// `adj_edges[4]` receives the local edge index within those faces.
    ///
    /// Mirrors C++ `PtexIndices::GetAdjacency(...)`.
    #[doc(alias = "GetAdjacency")]
    pub fn get_adjacency(
        &self,
        refiner: &TopologyRefiner,
        face: i32,
        quadrant: i32,
        adj_faces: &mut [i32; 4],
        adj_edges: &mut [i32; 4],
    ) {
        let level = refiner.get_level(0);
        let fedges = level.get_face_edges(face as Index);

        if fedges.size() == self.reg_face_size {
            // Regular ptex quad face — one Ptex face per coarse face
            for i in 0..self.reg_face_size as usize {
                let edge = fedges[i as i32];
                let adj_face = get_adjacent_face(&level, edge, face as Index);
                if adj_face < 0 {
                    adj_faces[i] = -1;
                    adj_edges[i] = 0;
                } else {
                    let aedges = level.get_face_edges(adj_face);
                    if aedges.size() == self.reg_face_size {
                        adj_faces[i] = self.ptex_indices[adj_face as usize];
                        let local = aedges.find_index(edge);
                        adj_edges[i] = local;
                    } else {
                        // Neighbor is an n-gon sub-face
                        let local = aedges.find_index(edge);
                        adj_faces[i] =
                            self.ptex_indices[adj_face as usize] + (local + 1) % aedges.size();
                        adj_edges[i] = 3;
                    }
                }
            }
            if self.reg_face_size == 3 {
                // Loop: fourth slot unused
                adj_faces[3] = -1;
                adj_edges[3] = 0;
            }
        } else if self.reg_face_size == 4 {
            // Catmark n-gon → virtual sub-quad 'quadrant'
            let nq = fedges.size();
            let next = (quadrant + 1) % nq;
            let prev = (quadrant + nq - 1) % nq;

            // Inner neighbors (edges 1 & 2 of the virtual sub-quad)
            adj_faces[1] = self.ptex_indices[face as usize] + next;
            adj_edges[1] = 2;
            adj_faces[2] = self.ptex_indices[face as usize] + prev;
            adj_edges[2] = 1;

            // Outer neighbor along edge 0
            let edge0 = fedges[quadrant];
            let adj_face0 = get_adjacent_face(&level, edge0, face as Index);
            if adj_face0 < 0 {
                adj_faces[0] = -1;
                adj_edges[0] = 0;
            } else {
                let afedges = level.get_face_edges(adj_face0);
                if afedges.size() == 4 {
                    adj_faces[0] = self.ptex_indices[adj_face0 as usize];
                    adj_edges[0] = afedges.find_index_in_4_tuple(edge0);
                } else {
                    let sub = (afedges.find_index(edge0) + 1) % afedges.size();
                    adj_faces[0] = self.ptex_indices[adj_face0 as usize] + sub;
                    adj_edges[0] = 3;
                }
            }

            // Outer neighbor along edge 3
            let edge3 = fedges[prev];
            let adj_face3 = get_adjacent_face(&level, edge3, face as Index);
            if adj_face3 < 0 {
                adj_faces[3] = -1;
                adj_edges[3] = 0;
            } else {
                let afedges = level.get_face_edges(adj_face3);
                if afedges.size() == 4 {
                    adj_faces[3] = self.ptex_indices[adj_face3 as usize];
                    adj_edges[3] = afedges.find_index_in_4_tuple(edge3);
                } else {
                    let sub = afedges.find_index(edge3);
                    adj_faces[3] = self.ptex_indices[adj_face3 as usize] + sub;
                    adj_edges[3] = 0;
                }
            }
        } else {
            // Non-quad scheme (Loop) with irregular face: Ptex adjacency undefined.
            // Mirrors C++ Far::Error(FAR_RUNTIME_ERROR, ...) in ptexIndices.cpp.
            far_error(
                ErrorType::RuntimeError,
                "Ptex adjacency is not supported for non-quad schemes with irregular faces",
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Internal helper
// ---------------------------------------------------------------------------

/// Returns the face adjacent to `face` along `edge`, or -1 for boundary.
///
/// Mirrors the anonymous `getAdjacentFace` in C++ ptexIndices.cpp.
fn get_adjacent_face(
    level: &super::topology_level::TopologyLevel,
    edge: Index,
    face: Index,
) -> Index {
    let adj_faces = level.get_edge_faces(edge);
    if adj_faces.size() != 2 {
        return -1;
    }
    if adj_faces[0] == face {
        adj_faces[1]
    } else {
        adj_faces[0]
    }
}
