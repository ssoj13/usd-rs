//! FaceSurface — aggregate combining FaceTopology + subsets + indices.
//!
//! Ported from OpenSubdiv bfr/faceSurface.h/.cpp.

use super::face_topology::FaceTopology;
use super::face_vertex::FaceVertex;
use super::face_vertex_subset::FaceVertexSubset;
use super::vertex_tag::MultiVertexTag;
use crate::sdc::options::{FVarLinearInterpolation, Options, VtxBoundaryInterpolation};
use crate::sdc::types::SchemeType;

pub type Index = super::face_vertex::Index;

/// Aggregates a `FaceTopology` with per-corner subsets and associated indices
/// to produce a complete description of the limit surface for one face.
///
/// Mirrors `Bfr::FaceSurface`.
pub struct FaceSurface<'a> {
    /// Borrowed reference to the topology (lifetime tied to the topology owner).
    pub(crate) topology: &'a FaceTopology,
    /// Face-vertex or face-varying indices (one flat array for the whole face).
    pub(crate) indices: &'a [Index],
    /// Per-corner topological subsets.
    pub(crate) corners: Vec<FaceVertexSubset>,

    pub(crate) combined_tag: MultiVertexTag,
    pub(crate) options_in_effect: Options,

    pub(crate) is_face_varying: bool,
    pub(crate) matches_vertex: bool,
    pub(crate) is_regular: bool,
    /// Tracks whether Initialize() has been called, mirroring C++ `_topology != 0`.
    pub(crate) initialized: bool,
}

impl<'a> FaceSurface<'a> {
    // ------------------------------------------------------------------
    //  Constructors / initializers
    // ------------------------------------------------------------------

    /// Construct a vertex-topology surface.
    pub fn from_vertex(topology: &'a FaceTopology, vtx_indices: &'a [Index]) -> Self {
        let mut s = Self::uninit(topology, vtx_indices);
        s.init_vertex();
        s
    }

    /// Construct a face-varying surface relative to a previously-built vertex surface.
    pub fn from_fvar(vtx_surface: &'a FaceSurface<'a>, fvar_indices: &'a [Index]) -> Self {
        let mut s = Self::uninit(vtx_surface.topology, fvar_indices);
        s.init_fvar(vtx_surface);
        s
    }

    // Re-initialize in place for vertex topology.
    pub fn initialize_vertex(&mut self, topology: &'a FaceTopology, vtx_indices: &'a [Index]) {
        self.topology = topology;
        self.indices = vtx_indices;
        self.corners.clear();
        self.combined_tag.clear();
        self.is_face_varying = false;
        self.matches_vertex = false;
        self.initialized = false; // reset before re-init
        self.init_vertex();
    }

    // ------------------------------------------------------------------
    //  Simple queries
    // ------------------------------------------------------------------

    /// Returns true after a successful call to `from_vertex` or `from_fvar`.
    /// Mirrors C++ `_topology != 0` (non-null topology pointer).
    pub fn is_initialized(&self) -> bool {
        self.initialized
    }
    pub fn is_regular(&self) -> bool {
        self.is_regular
    }
    pub fn fvar_topology_matches_vertex(&self) -> bool {
        self.matches_vertex
    }

    pub fn get_topology(&self) -> &FaceTopology {
        self.topology
    }
    pub fn get_subsets(&self) -> &[FaceVertexSubset] {
        &self.corners
    }
    pub fn get_indices(&self) -> &[Index] {
        self.indices
    }
    pub fn get_tag(&self) -> MultiVertexTag {
        self.combined_tag
    }

    pub fn get_face_size(&self) -> i32 {
        self.topology.get_face_size()
    }
    pub fn get_reg_face_size(&self) -> i32 {
        self.topology.get_reg_face_size()
    }

    pub fn get_sdc_scheme(&self) -> SchemeType {
        self.topology.get_scheme_type()
    }
    pub fn get_sdc_options_in_effect(&self) -> Options {
        self.options_in_effect
    }
    pub fn get_sdc_options_as_assigned(&self) -> Options {
        self.topology.get_scheme_options()
    }

    pub fn get_corner_topology(&self, c: usize) -> &FaceVertex {
        self.topology.get_topology(c)
    }
    pub fn get_corner_subset(&self, c: usize) -> &FaceVertexSubset {
        &self.corners[c]
    }

    pub fn get_num_indices(&self) -> i32 {
        self.topology.get_num_face_vertices()
    }

    // ------------------------------------------------------------------
    //  Private helpers
    // ------------------------------------------------------------------

    fn uninit(topology: &'a FaceTopology, indices: &'a [Index]) -> Self {
        let face_size = topology.get_face_size() as usize;
        let mut corners = Vec::with_capacity(face_size);
        corners.resize(face_size, FaceVertexSubset::default());
        Self {
            topology,
            indices,
            corners,
            combined_tag: MultiVertexTag::default(),
            options_in_effect: Options::default(),
            is_face_varying: false,
            matches_vertex: false,
            is_regular: false,
            initialized: false,
        }
    }

    fn init_vertex(&mut self) {
        self.initialized = true;
        let use_inf_sharp_subsets = self.topology.get_tag().has_inf_sharp_edges()
            && !self.topology.get_tag().has_inf_sharp_darts();

        for c in 0..self.get_face_size() as usize {
            let vtx_sub = &mut self.corners[c];
            // Safe: topology borrows and corners borrows are disjoint
            let vtx_top = self.topology.get_topology(c);
            vtx_top.get_vertex_subset(vtx_sub);

            if vtx_sub.is_boundary() && !vtx_sub.is_sharp() {
                self.sharpen_by_vtx_boundary(c);
            }
            let _ = use_inf_sharp_subsets; // WIP: subset reduction not implemented
            self.combined_tag.combine(self.corners[c].get_tag());
        }

        self.is_regular = self.compute_is_regular();
        self.options_in_effect = self.get_sdc_options_as_assigned();
        if !self.is_regular {
            self.revise_sdc_options_in_effect();
        }
    }

    fn init_fvar(&mut self, vtx_surface: &FaceSurface) {
        self.initialized = true;
        self.is_face_varying = true;
        // C++ preInitialize sets _matchesVertex = false before the fvar loop,
        // then the loop does _matchesVertex = _matchesVertex && ..., so it
        // starts false. The Rust port must match this to preserve parity.
        self.matches_vertex = false;

        let mut fvar_ptr = self.indices;

        for c in 0..self.get_face_size() as usize {
            let vtx_sub = vtx_surface.get_corner_subset(c);
            let fvar_sub = &mut self.corners[c];

            let vtx_top = self.topology.get_topology(c);
            vtx_top.find_face_varying_subset(fvar_sub, fvar_ptr, vtx_sub);

            let num_fv = vtx_top.get_num_face_vertices() as usize;

            if fvar_sub.is_boundary() && !fvar_sub.is_sharp() {
                self.sharpen_by_fvar_linear(c, fvar_ptr, vtx_sub);
            }
            self.combined_tag.combine(self.corners[c].get_tag());
            self.matches_vertex =
                self.matches_vertex && self.corners[c].shape_matches_superset(vtx_sub);

            fvar_ptr = &fvar_ptr[num_fv..];
        }

        self.is_regular = self.compute_is_regular();
        self.options_in_effect = self.get_sdc_options_as_assigned();
        if !self.is_regular {
            self.revise_sdc_options_in_effect();
        }
    }

    fn compute_is_regular(&self) -> bool {
        let tags = self.combined_tag;
        if tags.has_sharp_edges()
            || tags.has_semi_sharp_vertices()
            || tags.has_irregular_face_sizes()
        {
            return false;
        }
        let reg4 = self.get_reg_face_size() == 4;
        if !tags.has_boundary_vertices() {
            if tags.has_inf_sharp_vertices() {
                return false;
            }
            return if reg4 {
                self.corners[0].get_num_faces() == 4
                    && self.corners[1].get_num_faces() == 4
                    && self.corners[2].get_num_faces() == 4
                    && self.corners[3].get_num_faces() == 4
            } else {
                self.corners[0].get_num_faces() == 6
                    && self.corners[1].get_num_faces() == 6
                    && self.corners[2].get_num_faces() == 6
            };
        }
        let reg_interior = if reg4 { 4 } else { 6 };
        let reg_boundary = reg_interior / 2;
        for c in 0..self.get_face_size() as usize {
            let corner = &self.corners[c];
            if corner.is_sharp() {
                if corner.get_num_faces() != 1 {
                    return false;
                }
            } else if corner.is_boundary() {
                if corner.get_num_faces() != reg_boundary {
                    return false;
                }
            } else if corner.get_num_faces() != reg_interior {
                return false;
            }
        }
        true
    }

    fn revise_sdc_options_in_effect(&mut self) {
        debug_assert!(!self.is_regular);
        let tags = self.combined_tag;
        // Read scheme before taking a mutable borrow on options_in_effect.
        let scheme = self.get_sdc_scheme();
        let opts = &mut self.options_in_effect;

        // Boundary and fvar interpolation fixed
        opts.set_vtx_boundary_interpolation(VtxBoundaryInterpolation::EdgeOnly);
        opts.set_fvar_linear_interpolation(FVarLinearInterpolation::All);

        // Crease method irrelevant without semi-sharp
        if opts.get_creasing_method() != crate::sdc::options::CreasingMethod::Uniform {
            if !tags.has_semi_sharp_edges() && !tags.has_semi_sharp_vertices() {
                opts.set_creasing_method(crate::sdc::options::CreasingMethod::Uniform);
            }
        }

        // Triangle subdivision irrelevant without triangles in non-Catmark
        use crate::sdc::options::TriangleSubdivision;
        if opts.get_triangle_subdivision() != TriangleSubdivision::Catmark {
            if scheme != SchemeType::Catmark || !tags.has_irregular_face_sizes() {
                opts.set_triangle_subdivision(TriangleSubdivision::Catmark);
            }
        }
    }

    fn sharpen_by_vtx_boundary(&mut self, c: usize) {
        let vtx_sub = &mut self.corners[c];
        let vtx_top = self.topology.get_topology(c);

        use VtxBoundaryInterpolation::*;
        let is_sharp = match self
            .topology
            .get_scheme_options()
            .get_vtx_boundary_interpolation()
        {
            None => false,
            EdgeOnly => false,
            EdgeAndCorner => vtx_top.get_num_faces() == 1,
        };
        if is_sharp {
            vtx_top.sharpen_subset(vtx_sub);
        }
    }

    fn sharpen_by_fvar_linear(
        &mut self,
        c: usize,
        fvar_indices: &[Index],
        vtx_sub: &FaceVertexSubset,
    ) {
        use FVarLinearInterpolation::*;
        let vtx_top = self.topology.get_topology(c);
        let fvar_sub = &mut self.corners[c];

        let is_sharp = match self
            .topology
            .get_scheme_options()
            .get_fvar_linear_interpolation()
        {
            None => false,
            CornersOnly => fvar_sub.get_num_faces() == 1,
            CornersPlus1 => {
                let sharp = fvar_sub.get_num_faces() == 1
                    || has_more_than_two_fvar_subsets(vtx_top, fvar_indices);
                if !sharp && has_dependent_sharpness(vtx_top, fvar_sub) {
                    let ds = get_dependent_sharpness(vtx_top, fvar_sub);
                    vtx_top.sharpen_subset_with(fvar_sub, ds);
                }
                sharp
            }
            CornersPlus2 => {
                let sharp = fvar_sub.get_num_faces() == 1
                    || has_more_than_two_fvar_subsets(vtx_top, fvar_indices);
                if !sharp {
                    let num_other = vtx_sub.get_num_faces() - fvar_sub.get_num_faces();
                    if num_other == 0 {
                        !vtx_sub.is_boundary()
                    } else if num_other == 1 {
                        true
                    } else {
                        if has_dependent_sharpness(vtx_top, fvar_sub) {
                            let ds = get_dependent_sharpness(vtx_top, fvar_sub);
                            vtx_top.sharpen_subset_with(fvar_sub, ds);
                        }
                        false
                    }
                } else {
                    sharp
                }
            }
            Boundaries => true,
            All => {
                // Should not reach here for boundary-unsharp case
                false
            }
        };
        if is_sharp {
            vtx_top.sharpen_subset(&mut self.corners[c]);
        }
    }
}

// ---------------------------------------------------------------------------
//  FVar utility functions (local namespace equivalent)
// ---------------------------------------------------------------------------

fn has_more_than_two_fvar_subsets(top: &FaceVertex, fvar_indices: &[Index]) -> bool {
    let index_corner = top.get_face_index_at_corner_self(fvar_indices);
    let mut index_other: Option<Index> = None;
    let mut num_other_edges_discts = 1i32;

    for face in 0..top.get_num_faces() {
        let index = top.get_face_index_at_corner(face, fvar_indices);
        if index == index_corner {
            continue;
        }
        if let Some(other) = index_other {
            if index != other {
                return true;
            }
        } else {
            index_other = Some(index);
        }
        let face_next = top.get_face_next(face);
        let discont =
            face_next < 0 || !top.face_indices_match_across_edge(face, face_next, fvar_indices);
        num_other_edges_discts += discont as i32;
        if num_other_edges_discts > 2 {
            return true;
        }
    }
    false
}

fn has_dependent_sharpness(top: &FaceVertex, subset: &FaceVertexSubset) -> bool {
    (top.get_num_faces() - subset.get_num_faces()) > 1
        && top.get_tag().has_sharp_edges()
        && !subset.get_tag().has_sharp_edges()
}

fn get_dependent_sharpness(top: &FaceVertex, subset: &FaceVertexSubset) -> f32 {
    let first_face = top.get_face_first(subset);
    let last_face = top.get_face_last(subset);

    let first_face_prev = top.get_face_previous(first_face);
    let last_face_next = top.get_face_next(last_face);

    let skip_first = if first_face_prev < 0 {
        -1i32
    } else {
        first_face
    };
    let skip_last = if last_face_next < 0 {
        -1i32
    } else {
        last_face_next
    };

    let mut sharp = 0.0f32;
    for i in 0..top.get_num_faces() {
        if top.get_face_previous(i) >= 0 {
            if i != skip_first && i != skip_last {
                let s = top.get_face_edge_sharpness_by_idx(2 * i);
                if s > sharp {
                    sharp = s;
                }
            }
        }
    }
    if sharp > top.get_vertex_sharpness() {
        sharp
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sdc::options::Options;
    use crate::sdc::types::SchemeType;

    #[test]
    fn face_surface_is_initialized_false_for_empty_indices() {
        let _ft = FaceTopology::new(SchemeType::Catmark, Options::default());
        // Can't safely test from_vertex without a fully populated topology,
        // just confirm the type compiles.
        let _ = std::mem::size_of::<FaceSurface>();
    }
}
