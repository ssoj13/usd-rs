//! FaceTopology — full topological neighbourhood around a base face.
//!
//! Ported from OpenSubdiv bfr/faceTopology.h/.cpp.

use crate::sdc::types::{SchemeType, SchemeTypeTraits};
use crate::sdc::options::Options;
use super::face_vertex::{FaceVertex, Index};
use super::vertex_tag::MultiVertexTag;

/// Full topological description of the neighbourhood around a face, comprising
/// one `FaceVertex` per corner.
///
/// Mirrors `Bfr::FaceTopology`.
pub struct FaceTopology {
    pub(crate) scheme_type:    SchemeType,
    pub(crate) scheme_options: Options,

    pub(crate) face_size:           i32,
    pub(crate) reg_face_size:       i32,
    pub(crate) num_face_verts_total: i32,

    pub(crate) combined_tag: MultiVertexTag,

    pub(crate) is_initialized: bool,
    pub(crate) is_finalized:   bool,

    pub(crate) corner: Vec<FaceVertex>,
}

impl FaceTopology {
    /// Construct for a given subdivision scheme and options.
    pub fn new(scheme_type: SchemeType, scheme_options: Options) -> Self {
        let reg_face_size = SchemeTypeTraits::regular_face_size(scheme_type) as i32;
        Self {
            scheme_type,
            scheme_options,
            face_size:           0,
            reg_face_size,
            num_face_verts_total: 0,
            combined_tag: MultiVertexTag::default(),
            is_initialized: false,
            is_finalized:   false,
            corner: Vec::new(),
        }
    }

    // ------------------------------------------------------------------
    //  Accessors
    // ------------------------------------------------------------------

    pub fn get_scheme_type(&self)    -> SchemeType { self.scheme_type }
    pub fn get_scheme_options(&self) -> Options    { self.scheme_options }

    pub fn get_face_size(&self)     -> i32 { self.face_size }
    pub fn get_reg_face_size(&self) -> i32 { self.reg_face_size }

    pub fn get_topology(&self, i: usize) -> &FaceVertex { &self.corner[i] }
    pub fn get_topology_mut(&mut self, i: usize) -> &mut FaceVertex { &mut self.corner[i] }

    pub fn get_tag(&self) -> MultiVertexTag { self.combined_tag }

    pub fn get_num_face_vertices(&self) -> i32 { self.num_face_verts_total }
    pub fn get_num_face_vertices_at(&self, i: usize) -> i32 {
        self.corner[i].get_num_face_vertices()
    }

    pub fn has_un_ordered_corners(&self) -> bool {
        self.combined_tag.has_un_ordered_vertices()
    }

    // ------------------------------------------------------------------
    //  Initialize / Finalize
    // ------------------------------------------------------------------

    /// Prepare for a face of the given size; allocates the corner array.
    pub fn initialize(&mut self, face_size: i32) {
        self.face_size            = face_size;
        self.num_face_verts_total = 0;
        self.combined_tag.clear();
        self.is_initialized = true;
        self.is_finalized   = false;
        self.corner.resize_with(face_size as usize, FaceVertex::new);
    }

    /// Accumulate combined tags from all corners.
    pub fn finalize(&mut self) {
        debug_assert!(self.is_initialized);
        for i in 0..self.face_size as usize {
            self.combined_tag.combine(self.corner[i].get_tag());
            self.num_face_verts_total += self.corner[i].get_num_face_vertices();
        }
        self.is_finalized = true;
    }

    /// Connect any corners that declared their incident faces as unordered,
    /// using the flat array of face-vertex indices for all corners combined.
    pub fn resolve_un_ordered_corners(&mut self, fv_indices: &[Index]) {
        self.combined_tag.clear();
        let mut offset = 0usize;
        for i in 0..self.face_size as usize {
            if self.corner[i].get_tag().is_un_ordered() {
                let num_fv = self.corner[i].get_num_face_vertices() as usize;
                self.corner[i].connect_un_ordered_faces(&fv_indices[offset..offset + num_fv]);
            }
            self.combined_tag.combine(self.corner[i].get_tag());
            offset += self.corner[i].get_num_face_vertices() as usize;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn face_topology_initialize() {
        let mut ft = FaceTopology::new(SchemeType::Catmark, Options::default());
        ft.initialize(4);
        assert_eq!(ft.get_face_size(), 4);
        assert_eq!(ft.get_reg_face_size(), 4);
        assert!(!ft.has_un_ordered_corners());
    }
}
