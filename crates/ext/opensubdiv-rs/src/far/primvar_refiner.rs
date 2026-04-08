// Copyright 2015 DreamWorks Animation LLC.
// Ported to Rust from OpenSubdiv 3.7.0 far/primvarRefiner.h

//! Primvar interpolation across subdivision levels.
//!
//! [`PrimvarRefiner`] applies subdivision masks to user-supplied primvar
//! buffers.  The buffer type must implement [`Interpolatable`].
//!
//! Mirrors C++ `Far::PrimvarRefinerReal<REAL>`.

use super::topology_refiner::TopologyRefiner;
use crate::sdc::{
    Options,
    bilinear_scheme::BilinearKernel,
    catmark_scheme::CatmarkKernel,
    crease::{Crease, Rule},
    loop_scheme::LoopKernel,
    scheme::{EdgeNeighborhood, FaceNeighborhood, MaskInterface, Scheme, VertexNeighborhood},
    types::SchemeType,
};
use crate::vtr::types::{INDEX_INVALID, Index};
use crate::vtr::{Level, Refinement};

// ---------------------------------------------------------------------------
// Interpolatable — the required trait for user primvar buffers
// ---------------------------------------------------------------------------

/// Required interface for a primvar buffer element.
///
/// Mirrors the `Clear()` / `AddWithWeight()` interface in C++.
pub trait Interpolatable: Sized + Clone + Default {
    /// Zero this element.
    fn clear(&mut self);
    /// Accumulate: `self += src * weight`.
    fn add_with_weight(&mut self, src: &Self, weight: f32);
}

impl Interpolatable for f32 {
    fn clear(&mut self) {
        *self = 0.0;
    }
    fn add_with_weight(&mut self, src: &f32, weight: f32) {
        *self += src * weight;
    }
}

impl Interpolatable for f64 {
    fn clear(&mut self) {
        *self = 0.0;
    }
    fn add_with_weight(&mut self, src: &f64, weight: f32) {
        *self += src * weight as f64;
    }
}

impl Interpolatable for [f32; 2] {
    fn clear(&mut self) {
        *self = [0.0; 2];
    }
    fn add_with_weight(&mut self, src: &[f32; 2], weight: f32) {
        self[0] += src[0] * weight;
        self[1] += src[1] * weight;
    }
}

impl Interpolatable for [f32; 3] {
    fn clear(&mut self) {
        *self = [0.0; 3];
    }
    fn add_with_weight(&mut self, src: &[f32; 3], weight: f32) {
        self[0] += src[0] * weight;
        self[1] += src[1] * weight;
        self[2] += src[2] * weight;
    }
}

impl Interpolatable for [f32; 4] {
    fn clear(&mut self) {
        *self = [0.0; 4];
    }
    fn add_with_weight(&mut self, src: &[f32; 4], weight: f32) {
        self[0] += src[0] * weight;
        self[1] += src[1] * weight;
        self[2] += src[2] * weight;
        self[3] += src[3] * weight;
    }
}

// ---------------------------------------------------------------------------
// WeightMask — local scratch mask implementing MaskInterface
// ---------------------------------------------------------------------------

struct WeightMask {
    v_weights: Vec<f32>,
    e_weights: Vec<f32>,
    f_weights: Vec<f32>,
    f_for_centers: bool,
}

impl WeightMask {
    fn new(max_valence: usize) -> Self {
        Self {
            v_weights: vec![0.0; 1],
            e_weights: vec![0.0; max_valence],
            f_weights: vec![0.0; max_valence],
            f_for_centers: false,
        }
    }

    fn clear_all(&mut self) {
        for w in self.v_weights.iter_mut() {
            *w = 0.0;
        }
        for w in self.e_weights.iter_mut() {
            *w = 0.0;
        }
        for w in self.f_weights.iter_mut() {
            *w = 0.0;
        }
        self.f_for_centers = false;
    }
}

impl MaskInterface for WeightMask {
    fn num_vertex_weights(&self) -> usize {
        self.v_weights.len()
    }
    fn num_edge_weights(&self) -> usize {
        self.e_weights.len()
    }
    fn num_face_weights(&self) -> usize {
        self.f_weights.len()
    }

    fn set_num_vertex_weights(&mut self, n: usize) {
        self.v_weights.resize(n, 0.0);
    }
    fn set_num_edge_weights(&mut self, n: usize) {
        self.e_weights.resize(n, 0.0);
    }
    fn set_num_face_weights(&mut self, n: usize) {
        self.f_weights.resize(n, 0.0);
    }

    fn vertex_weight(&self, i: usize) -> f32 {
        self.v_weights[i]
    }
    fn edge_weight(&self, i: usize) -> f32 {
        self.e_weights[i]
    }
    fn face_weight(&self, i: usize) -> f32 {
        self.f_weights[i]
    }

    fn set_vertex_weight(&mut self, i: usize, w: f32) {
        self.v_weights[i] = w;
    }
    fn set_edge_weight(&mut self, i: usize, w: f32) {
        self.e_weights[i] = w;
    }
    fn set_face_weight(&mut self, i: usize, w: f32) {
        self.f_weights[i] = w;
    }

    fn face_weights_for_face_centers(&self) -> bool {
        self.f_for_centers
    }
    fn set_face_weights_for_face_centers(&mut self, v: bool) {
        self.f_for_centers = v;
    }
}

// ---------------------------------------------------------------------------
// Topology neighbourhood adapters
// ---------------------------------------------------------------------------

struct FaceNbr(usize);
impl FaceNeighborhood for FaceNbr {
    fn num_vertices(&self) -> usize {
        self.0
    }
}

struct EdgeNbr {
    sharpness: f32,
    num_faces: usize,
    child_sharpnesses: [f32; 2],
}
impl EdgeNeighborhood for EdgeNbr {
    fn num_faces(&self) -> usize {
        self.num_faces
    }
    fn sharpness(&self) -> f32 {
        self.sharpness
    }
    fn num_vertices_per_face(&self, _counts: &mut [usize]) {}
    fn child_sharpnesses(&self, _crease: &Crease, out: &mut [f32; 2]) {
        *out = self.child_sharpnesses;
    }
}

struct VertNbr {
    sharpness: f32,
    num_edges: usize,
    num_faces: usize,
    edge_sharp: Vec<f32>,
    child_sharp: f32,
    child_e_sharp: Vec<f32>,
}
impl VertexNeighborhood for VertNbr {
    fn num_edges(&self) -> usize {
        self.num_edges
    }
    fn num_faces(&self) -> usize {
        self.num_faces
    }
    fn sharpness(&self) -> f32 {
        self.sharpness
    }
    fn sharpness_per_edge<'b>(&self, out: &'b mut [f32]) -> &'b [f32] {
        let n = self.num_edges.min(out.len());
        out[..n].copy_from_slice(&self.edge_sharp[..n]);
        &out[..n]
    }
    fn child_sharpness(&self, _crease: &Crease) -> f32 {
        self.child_sharp
    }
    fn child_sharpness_per_edge<'b>(&self, _crease: &Crease, out: &'b mut [f32]) -> &'b [f32] {
        let n = self.num_edges.min(out.len());
        out[..n].copy_from_slice(&self.child_e_sharp[..n]);
        &out[..n]
    }
}

// ---------------------------------------------------------------------------
// PrimvarRefiner
// ---------------------------------------------------------------------------

/// Applies subdivision weights to generic primvar data.
///
/// Mirrors C++ `Far::PrimvarRefinerReal<REAL>`.
pub struct PrimvarRefiner<'r> {
    refiner: &'r TopologyRefiner,
}

impl<'r> PrimvarRefiner<'r> {
    pub fn new(refiner: &'r TopologyRefiner) -> Self {
        Self { refiner }
    }

    pub fn get_topology_refiner(&self) -> &TopologyRefiner {
        self.refiner
    }

    fn scheme_type(&self) -> SchemeType {
        self.refiner.get_scheme_type()
    }
    fn scheme_options(&self) -> Options {
        self.refiner.get_scheme_options()
    }

    // -----------------------------------------------------------------------
    // Public API
    // -----------------------------------------------------------------------

    /// Apply vertex interpolation weights for refinement `level` (1-based).
    pub fn interpolate<T: Interpolatable>(&self, level: i32, src: &[T], dst: &mut [T]) {
        match self.scheme_type() {
            SchemeType::Bilinear => self.interp_scheme::<T, BilinearKernel>(level, src, dst),
            SchemeType::Catmark => self.interp_scheme::<T, CatmarkKernel>(level, src, dst),
            SchemeType::Loop => self.interp_scheme::<T, LoopKernel>(level, src, dst),
        }
    }

    /// Apply varying interpolation weights for refinement `level`.
    pub fn interpolate_varying<T: Interpolatable>(&self, level: i32, src: &[T], dst: &mut [T]) {
        let refinement = self.refiner.get_refinement_internal(level - 1);
        let parent = self.refiner.get_level_internal(level - 1);

        // Face-verts: centroid of face corners
        let nf = refinement.get_num_child_vertices_from_faces();
        let base = refinement.get_first_child_vertex_from_faces();
        for i in 0..nf {
            let cv = base + i;
            let pface = refinement.get_child_vertex_parent_index(cv);
            let fverts = parent.get_face_vertices(pface);
            let n = fverts.size();
            let w = 1.0 / n as f32;
            dst[cv as usize].clear();
            for k in 0..n {
                dst[cv as usize].add_with_weight(&src[fverts[k] as usize], w);
            }
        }

        // Edge-verts: midpoint
        let ne = refinement.get_num_child_vertices_from_edges();
        let ebase = refinement.get_first_child_vertex_from_edges();
        for i in 0..ne {
            let cv = ebase + i;
            let pedge = refinement.get_child_vertex_parent_index(cv);
            let ev = parent.get_edge_vertices(pedge);
            dst[cv as usize].clear();
            dst[cv as usize].add_with_weight(&src[ev[0] as usize], 0.5);
            dst[cv as usize].add_with_weight(&src[ev[1] as usize], 0.5);
        }

        // Vert-verts: pass-through
        let nv = refinement.get_num_child_vertices_from_vertices();
        let vbase = refinement.get_first_child_vertex_from_vertices();
        for i in 0..nv {
            let cv = vbase + i;
            let pvert = refinement.get_child_vertex_parent_index(cv);
            dst[cv as usize].clear();
            dst[cv as usize].add_with_weight(&src[pvert as usize], 1.0);
        }
    }

    /// Apply face-uniform (per-face-centre) interpolation for refinement `level`.
    pub fn interpolate_face_uniform<T: Interpolatable>(
        &self,
        level: i32,
        src: &[T],
        dst: &mut [T],
    ) {
        let refinement = self.refiner.get_refinement_internal(level - 1);
        let parent = self.refiner.get_level_internal(level - 1);
        let nf = parent.get_num_faces();
        for pf in 0..nf {
            let cv = refinement.get_face_child_vertex(pf);
            if cv == INDEX_INVALID {
                continue;
            }
            dst[cv as usize].clear();
            dst[cv as usize].add_with_weight(&src[pf as usize], 1.0);
        }
    }

    /// Apply face-varying interpolation weights for refinement `level`.
    pub fn interpolate_face_varying<T: Interpolatable>(
        &self,
        level: i32,
        src: &[T],
        dst: &mut [T],
        channel: i32,
    ) {
        match self.scheme_type() {
            SchemeType::Bilinear => {
                self.interp_fvar_scheme::<T, BilinearKernel>(level, src, dst, channel)
            }
            SchemeType::Catmark => {
                self.interp_fvar_scheme::<T, CatmarkKernel>(level, src, dst, channel)
            }
            SchemeType::Loop => self.interp_fvar_scheme::<T, LoopKernel>(level, src, dst, channel),
        }
    }

    /// Evaluate limit surface positions for all vertices at the finest level.
    pub fn limit<T: Interpolatable>(&self, src: &[T], dst_pos: &mut [T]) {
        match self.scheme_type() {
            SchemeType::Bilinear => self.limit_impl::<T, BilinearKernel>(src, dst_pos, None, None),
            SchemeType::Catmark => self.limit_impl::<T, CatmarkKernel>(src, dst_pos, None, None),
            SchemeType::Loop => self.limit_impl::<T, LoopKernel>(src, dst_pos, None, None),
        }
    }

    /// Evaluate limit positions and first-derivative tangents.
    pub fn limit_with_tangents<T: Interpolatable>(
        &self,
        src: &[T],
        dst_pos: &mut [T],
        dst_tan1: &mut [T],
        dst_tan2: &mut [T],
    ) {
        match self.scheme_type() {
            SchemeType::Bilinear => {
                self.limit_impl::<T, BilinearKernel>(src, dst_pos, Some(dst_tan1), Some(dst_tan2))
            }
            SchemeType::Catmark => {
                self.limit_impl::<T, CatmarkKernel>(src, dst_pos, Some(dst_tan1), Some(dst_tan2))
            }
            SchemeType::Loop => {
                self.limit_impl::<T, LoopKernel>(src, dst_pos, Some(dst_tan1), Some(dst_tan2))
            }
        }
    }

    /// Evaluate face-varying limit values at the finest level.
    ///
    /// Mirrors C++ `PrimvarRefiner::LimitFaceVarying()` / `limitFVar<SCHEME>`.
    ///
    /// For vertices where fvar topology matches the vertex topology, the
    /// scheme limit mask is applied to fvar values (same formula as vertex
    /// limit but indexing fvar values instead of vertices). For mismatched
    /// vertices (seams/corners), the value is treated as a crease endpoint:
    /// `0.75 * center + 0.125 * (end0 + end1)`, or a corner (pass-through).
    /// Since we do not have direct access to the internal FVarLevel here,
    /// we use the public fvar topology match query and apply the scheme limit
    /// mask via the same vertex-neighborhood path used for vertex limit.
    pub fn limit_face_varying<T: Interpolatable>(&self, src: &[T], dst: &mut [T], channel: i32) {
        match self.scheme_type() {
            crate::sdc::types::SchemeType::Bilinear => {
                self.limit_fvar_scheme::<T, BilinearKernel>(src, dst, channel)
            }
            crate::sdc::types::SchemeType::Catmark => {
                self.limit_fvar_scheme::<T, CatmarkKernel>(src, dst, channel)
            }
            crate::sdc::types::SchemeType::Loop => {
                self.limit_fvar_scheme::<T, LoopKernel>(src, dst, channel)
            }
        }
    }

    fn limit_fvar_scheme<T, K>(&self, src: &[T], dst: &mut [T], channel: i32)
    where
        T: Interpolatable,
        K: crate::sdc::scheme::SchemeKernel,
    {
        let max_level = self.refiner.get_max_level();
        let level = self.refiner.get_level_internal(max_level);
        let opts = self.scheme_options();
        let scheme = Scheme::<K>::with_options(opts);
        let crease = Crease::with_options(opts);
        let nv = level.get_num_vertices();
        let max_v = level.get_max_valence() as usize;
        let fvar = level.get_fvar_level(channel);

        let mut e_sh = vec![0.0f32; max_v];
        let mut ce_sh = vec![0.0f32; max_v];

        for vert in 0..nv {
            let vedges = level.get_vertex_edges(vert);
            let vvalues = fvar.get_vertex_values(vert);

            // Incomplete (sparse refinement) vertices or linear fvar channels:
            // pass all sibling values through unchanged — C++ limitFVar:1169-1177.
            let vtag = level.get_vertex_tag(vert);
            if vtag.incomplete() || vedges.size() == 0 || fvar.is_linear() {
                for i in 0..vvalues.size() as usize {
                    let vv = vvalues[i as i32] as usize;
                    dst[vv].clear();
                    dst[vv].add_with_weight(&src[vv], 1.0);
                }
                continue;
            }

            let vsharp = level.get_vertex_sharpness(vert);
            let vfaces = level.get_vertex_faces(vert);
            let vf_local = level.get_vertex_face_local_indices(vert);
            let ne = vedges.size() as usize;
            let nf = vfaces.size() as usize;

            if level.does_vertex_fvar_topology_match(vert, channel) {
                // Topology matches: apply scheme limit mask to fvar values
                // instead of vertex positions. The fvar value for this vertex
                // is found via the first incident face (same index as the vertex).
                e_sh.resize(ne, 0.0);
                for k in 0..ne {
                    e_sh[k] = level.get_edge_sharpness(vedges[k as i32]);
                }
                ce_sh.resize(ne, 0.0);
                crease.subdivide_edge_sharpnesses_around_vertex(&e_sh[..ne], &mut ce_sh[..ne]);
                let cvsh = crease.subdivide_vertex_sharpness(vsharp);

                let nbr = VertNbr {
                    sharpness: vsharp,
                    num_edges: ne,
                    num_faces: nf,
                    edge_sharp: e_sh[..ne].to_vec(),
                    child_sharp: cvsh,
                    child_e_sharp: ce_sh[..ne].to_vec(),
                };

                let mut pos_mask = WeightMask::new(max_v);
                pos_mask.clear_all();
                scheme.compute_vertex_limit_mask(&nbr, &mut pos_mask, level.get_vertex_rule(vert));

                // Find the fvar value index for this vertex from first incident face
                let fvar_vert_val = if vfaces.size() > 0 {
                    let face = vfaces[0];
                    let local = vf_local[0] as i32;
                    let fvals = level.get_face_fvar_values(face, channel);
                    if local < fvals.size() as i32 {
                        fvals[local]
                    } else {
                        vert
                    }
                } else {
                    vert
                };

                dst[fvar_vert_val as usize].clear();

                // face weights — use diagonal fvar value from each incident face
                for k in 0..nf.min(pos_mask.num_face_weights()) {
                    let fw = pos_mask.face_weight(k);
                    if fw == 0.0 {
                        continue;
                    }
                    let pface = vfaces[k as i32];
                    let pfvals = level.get_face_fvar_values(pface, channel);
                    let vin_k = vf_local[k as i32] as i32;
                    // opposite fvar value: (vInFace + 2) % faceSize
                    let mut opp_idx = vin_k + 2;
                    if opp_idx >= pfvals.size() as i32 {
                        opp_idx -= pfvals.size() as i32;
                    }
                    let opp_val = pfvals[opp_idx];
                    dst[fvar_vert_val as usize].add_with_weight(&src[opp_val as usize], fw);
                }

                // edge weights — use fvarChannel.getVertexEdgeValues, which correctly
                // handles sibling values at edge endpoints. Mirrors C++ limitFVar:1216-1222.
                let new_ew = pos_mask.num_edge_weights();
                if new_ew > 0 {
                    let mut v_edge_values = vec![0i32; ne];
                    fvar.get_vertex_edge_values(vert, &mut v_edge_values[..ne]);
                    for k in 0..ne.min(new_ew) {
                        let ew = pos_mask.edge_weight(k);
                        if ew == 0.0 {
                            continue;
                        }
                        dst[fvar_vert_val as usize]
                            .add_with_weight(&src[v_edge_values[k] as usize], ew);
                    }
                }

                // vertex weight — applied unconditionally, mirrors C++ limitFVar:1224.
                dst[fvar_vert_val as usize]
                    .add_with_weight(&src[fvar_vert_val as usize], pos_mask.vertex_weight(0));
            } else {
                // Mismatched topology (seam): each sibling value is independently
                // either a corner (pass-through) or a crease.
                // Crease formula: 1/6*end0 + 1/6*end1 + 2/3*center
                // Mirrors C++ primvarRefiner.h:1229-1242.
                for i in 0..vvalues.size() as usize {
                    let vvalue = vvalues[i as i32];
                    dst[vvalue as usize].clear();
                    if fvar.get_value_tag(vvalue).is_corner() {
                        dst[vvalue as usize].add_with_weight(&src[vvalue as usize], 1.0);
                    } else {
                        let mut end_vals = [0i32; 2];
                        fvar.get_vertex_crease_end_values(vert, i as u16, &mut end_vals);
                        dst[vvalue as usize].add_with_weight(&src[end_vals[0] as usize], 1.0 / 6.0);
                        dst[vvalue as usize].add_with_weight(&src[end_vals[1] as usize], 1.0 / 6.0);
                        dst[vvalue as usize].add_with_weight(&src[vvalue as usize], 2.0 / 3.0);
                    }
                }
            }
        }
    }

    // -----------------------------------------------------------------------
    // Scheme-specific interpolation
    // -----------------------------------------------------------------------

    fn interp_scheme<T, K>(&self, level: i32, src: &[T], dst: &mut [T])
    where
        T: Interpolatable,
        K: crate::sdc::scheme::SchemeKernel,
    {
        let refinement = self.refiner.get_refinement_internal(level - 1);
        let parent = self.refiner.get_level_internal(level - 1);
        let child = refinement.child();
        let scheme = Scheme::<K>::with_options(self.scheme_options());

        self.interp_from_faces::<T, K>(&scheme, refinement, parent, src, dst);
        self.interp_from_edges::<T, K>(&scheme, refinement, parent, child, src, dst);
        self.interp_from_verts::<T, K>(&scheme, refinement, parent, child, src, dst);
    }

    fn interp_from_faces<T, K>(
        &self,
        scheme: &Scheme<K>,
        refinement: &Refinement,
        parent: &Level,
        src: &[T],
        dst: &mut [T],
    ) where
        T: Interpolatable,
        K: crate::sdc::scheme::SchemeKernel,
    {
        let nf = refinement.get_num_child_vertices_from_faces();
        let base = refinement.get_first_child_vertex_from_faces();
        for i in 0..nf {
            let cv = base + i;
            let pface = refinement.get_child_vertex_parent_index(cv);
            let fverts = parent.get_face_vertices(pface);
            let nv = fverts.size() as usize;

            let mut mask = WeightMask::new(nv);
            mask.clear_all();
            scheme.compute_face_vertex_mask(&FaceNbr(nv), &mut mask);

            dst[cv as usize].clear();
            for k in 0..nv {
                let w = mask.vertex_weight(k);
                if w != 0.0 {
                    dst[cv as usize].add_with_weight(&src[fverts[k as i32] as usize], w);
                }
            }
        }
    }

    fn interp_from_edges<T, K>(
        &self,
        scheme: &Scheme<K>,
        refinement: &Refinement,
        parent: &Level,
        child: &Level,
        src: &[T],
        dst: &mut [T],
    ) where
        T: Interpolatable,
        K: crate::sdc::scheme::SchemeKernel,
    {
        let ne = refinement.get_num_child_vertices_from_edges();
        let base = refinement.get_first_child_vertex_from_edges();
        let crease = Crease::with_options(self.scheme_options());

        for i in 0..ne {
            let cv = base + i;
            let pedge = refinement.get_child_vertex_parent_index(cv);
            let everts = parent.get_edge_vertices(pedge);
            let efaces = parent.get_edge_faces(pedge);
            let sharp = parent.get_edge_sharpness(pedge);

            // Compute child edge sharpness via Crease
            let mut child_sh = [0.0f32; 2];
            child_sh[0] = crease.subdivide_uniform_sharpness(sharp);
            child_sh[1] = child_sh[0];

            let nf = efaces.size() as usize;
            let nbr = EdgeNbr {
                sharpness: sharp,
                num_faces: nf,
                child_sharpnesses: child_sh,
            };
            let mut mask = WeightMask::new(2 + nf);
            mask.clear_all();
            // C++: pRule = sharp edge → Crease, else Smooth; cRule from child level
            let p_rule = if sharp > 0.0 {
                Rule::Crease
            } else {
                Rule::Smooth
            };
            let c_rule = child.get_vertex_rule(cv);
            scheme.compute_edge_vertex_mask(&nbr, &mut mask, p_rule, c_rule);

            dst[cv as usize].clear();
            if mask.num_vertex_weights() >= 1 {
                let w0 = mask.vertex_weight(0);
                if w0 != 0.0 {
                    dst[cv as usize].add_with_weight(&src[everts[0] as usize], w0);
                }
            }
            if mask.num_vertex_weights() >= 2 {
                let w1 = mask.vertex_weight(1);
                if w1 != 0.0 {
                    dst[cv as usize].add_with_weight(&src[everts[1] as usize], w1);
                }
            }

            let nfw = mask.num_face_weights();
            for f in 0..nfw {
                let fw = mask.face_weight(f);
                if fw == 0.0 {
                    continue;
                }
                if f >= efaces.size() as usize {
                    continue;
                }
                let pface = efaces[f as i32];

                if mask.face_weights_for_face_centers() {
                    // Catmark smooth: use face-centre child vertex
                    let cfc = refinement.get_face_child_vertex(pface);
                    if cfc != INDEX_INVALID {
                        let val = dst[cfc as usize].clone();
                        dst[cv as usize].add_with_weight(&val, fw);
                    }
                } else {
                    // Loop: opposite vertex in that face
                    let pfv = parent.get_face_vertices(pface);
                    let opp = opposite_vert_across_edge(everts[0], everts[1], pfv.as_slice());
                    if opp != INDEX_INVALID {
                        dst[cv as usize].add_with_weight(&src[opp as usize], fw);
                    }
                }
            }
        }
    }

    fn interp_from_verts<T, K>(
        &self,
        scheme: &Scheme<K>,
        refinement: &Refinement,
        parent: &Level,
        child: &Level,
        src: &[T],
        dst: &mut [T],
    ) where
        T: Interpolatable,
        K: crate::sdc::scheme::SchemeKernel,
    {
        let nv = refinement.get_num_child_vertices_from_vertices();
        let base = refinement.get_first_child_vertex_from_vertices();
        let crease = Crease::with_options(self.scheme_options());
        let max_v = parent.get_max_valence() as usize;

        let mut edge_sharps = vec![0.0f32; max_v];
        let mut child_e_sharps = vec![0.0f32; max_v];

        for i in 0..nv {
            let cv = base + i;
            let pvert = refinement.get_child_vertex_parent_index(cv);
            let vedges = parent.get_vertex_edges(pvert);
            let vfaces = parent.get_vertex_faces(pvert);
            let ne = vedges.size() as usize;
            let nf = vfaces.size() as usize;
            let vsharp = parent.get_vertex_sharpness(pvert);

            // Gather edge sharpnesses
            edge_sharps.resize(ne, 0.0);
            for k in 0..ne {
                edge_sharps[k] = parent.get_edge_sharpness(vedges[k as i32]);
            }
            child_e_sharps.resize(ne, 0.0);
            crease.subdivide_edge_sharpnesses_around_vertex(
                &edge_sharps[..ne],
                &mut child_e_sharps[..ne],
            );
            let child_vsharp = crease.subdivide_vertex_sharpness(vsharp);

            let nbr = VertNbr {
                sharpness: vsharp,
                num_edges: ne,
                num_faces: nf,
                edge_sharp: edge_sharps[..ne].to_vec(),
                child_sharp: child_vsharp,
                child_e_sharp: child_e_sharps[..ne].to_vec(),
            };

            let mut mask = WeightMask::new(ne + 1);
            mask.clear_all();
            // C++: pRule from parent level, cRule from child level
            let p_rule = parent.get_vertex_rule(pvert);
            let c_rule = child.get_vertex_rule(cv);
            scheme.compute_vertex_vertex_mask(&nbr, &mut mask, p_rule, c_rule);

            dst[cv as usize].clear();
            let vw = if mask.num_vertex_weights() > 0 {
                mask.vertex_weight(0)
            } else {
                0.0
            };
            if vw != 0.0 {
                dst[cv as usize].add_with_weight(&src[pvert as usize], vw);
            }

            let new = mask.num_edge_weights();
            for k in 0..ne.min(new) {
                let ew = mask.edge_weight(k);
                if ew == 0.0 {
                    continue;
                }
                let e = vedges[k as i32];
                let ev = parent.get_edge_vertices(e);
                let opp = if ev[0] == pvert { ev[1] } else { ev[0] };
                dst[cv as usize].add_with_weight(&src[opp as usize], ew);
            }

            let nfw = mask.num_face_weights();
            for k in 0..nf.min(nfw) {
                let fw = mask.face_weight(k);
                if fw == 0.0 {
                    continue;
                }
                let pface = vfaces[k as i32];

                if mask.face_weights_for_face_centers() {
                    let cfc = refinement.get_face_child_vertex(pface);
                    if cfc != INDEX_INVALID {
                        let val = dst[cfc as usize].clone();
                        dst[cv as usize].add_with_weight(&val, fw);
                    }
                } else {
                    let pfv = parent.get_face_vertices(pface);
                    let opp = opp_vert_in_face_not(pvert, pfv.as_slice());
                    if opp != INDEX_INVALID {
                        dst[cv as usize].add_with_weight(&src[opp as usize], fw);
                    }
                }
            }
        }
    }

    // -----------------------------------------------------------------------
    // Face-varying interpolation — full C++ parity (primvarRefiner.h:636-1002)
    // -----------------------------------------------------------------------

    fn interp_fvar_scheme<T, K>(&self, level: i32, src: &[T], dst: &mut [T], channel: i32)
    where
        T: Interpolatable,
        K: crate::sdc::scheme::SchemeKernel,
    {
        let refinement = self.refiner.get_refinement_internal(level - 1);
        let parent = self.refiner.get_level_internal(level - 1);
        let child = refinement.child();
        let opts = self.scheme_options();
        let scheme = Scheme::<K>::with_options(opts);
        let crease = Crease::with_options(opts);
        let scheme_type = self.scheme_type();

        let parent_fvar = parent.get_fvar_level(channel);
        let child_fvar = child.get_fvar_level(channel);
        let refine_fvar = refinement.get_fvar_refinement(channel);

        // Linear fvar: parentFVar.isLinear() || scheme == Bilinear
        let is_linear_fvar = parent_fvar.is_linear() || scheme_type == SchemeType::Bilinear;

        let max_v = parent.get_max_valence() as usize;

        // ===================================================================
        // Phase 1 — interpFVarFromFaces (C++ 636-684)
        //
        // For each face-derived child vertex, write to the dst slot identified
        // by childFVar.getVertexValueOffset(cVert, 0), NOT cv directly.
        // The fvar values come from parentFVar.getFaceValues(pFace), not
        // parent.getFaceVertices().
        // ===================================================================
        {
            let nf = refinement.get_num_child_vertices_from_faces();
            let base = refinement.get_first_child_vertex_from_faces();
            for i in 0..nf {
                let cv = base + i;
                let pface = refinement.get_child_vertex_parent_index(cv);

                // dst index: childFVar value-offset for this face-child vertex
                let c_vert_value = child_fvar.get_vertex_value_offset(cv, 0);

                // src indices: parentFVar face values (NOT parent face vertices)
                let f_values = parent_fvar.get_face_values(pface);
                let nv = f_values.size() as usize;

                let mut mask = WeightMask::new(nv);
                mask.clear_all();
                scheme.compute_face_vertex_mask(&FaceNbr(nv), &mut mask);

                dst[c_vert_value as usize].clear();
                for k in 0..nv {
                    let w = mask.vertex_weight(k);
                    if w != 0.0 {
                        dst[c_vert_value as usize]
                            .add_with_weight(&src[f_values[k as i32] as usize], w);
                    }
                }
            }
        }

        // ===================================================================
        // Phase 2 — interpFVarFromEdges (C++ 686-829)
        // ===================================================================
        {
            let ne = refinement.get_num_child_vertices_from_edges();
            let base = refinement.get_first_child_vertex_from_edges();

            for i in 0..ne {
                let cv = base + i;
                let pedge = refinement.get_child_vertex_parent_index(cv);
                let sharp = parent.get_edge_sharpness(pedge);
                let efaces = parent.get_edge_faces(pedge);
                let nef = efaces.size() as usize;

                // child values for this edge-child vertex
                let c_vert_values = child_fvar.get_vertex_values(cv);
                // Check if topology matches (first sibling value)
                let matches_vertex = child_fvar.value_topology_matches(c_vert_values[0]);

                if matches_vertex {
                    // --- Matched topology: compute edge mask ---
                    let mut dyn_mask = WeightMask::new(2 + nef);
                    if is_linear_fvar {
                        // linear: fixed 0.5/0.5 endpoint weights
                        dyn_mask.set_num_vertex_weights(2);
                        dyn_mask.set_vertex_weight(0, 0.5);
                        dyn_mask.set_vertex_weight(1, 0.5);
                        dyn_mask.set_num_edge_weights(0);
                        dyn_mask.set_num_face_weights(0);
                    } else {
                        dyn_mask.clear_all();
                        let mut child_sh = [0.0f32; 2];
                        child_sh[0] = crease.subdivide_uniform_sharpness(sharp);
                        child_sh[1] = child_sh[0];
                        let nbr = EdgeNbr {
                            sharpness: sharp,
                            num_faces: nef,
                            child_sharpnesses: child_sh,
                        };
                        let p_rule = if sharp > 0.0 {
                            Rule::Crease
                        } else {
                            Rule::Smooth
                        };
                        let c_rule = child.get_vertex_rule(cv);
                        scheme.compute_edge_vertex_mask(&nbr, &mut dyn_mask, p_rule, c_rule);
                    }

                    let active_mask = &dyn_mask;

                    // get fvar values at the two edge endpoints for this face index (0)
                    let mut e_vert_values = [0i32; 2];
                    parent_fvar.get_edge_face_values(pedge, 0, &mut e_vert_values);

                    let c_vert_value = c_vert_values[0];
                    dst[c_vert_value as usize].clear();

                    let w0 = if active_mask.num_vertex_weights() >= 1 {
                        active_mask.vertex_weight(0)
                    } else {
                        0.5
                    };
                    let w1 = if active_mask.num_vertex_weights() >= 2 {
                        active_mask.vertex_weight(1)
                    } else {
                        0.5
                    };
                    if w0 != 0.0 {
                        dst[c_vert_value as usize]
                            .add_with_weight(&src[e_vert_values[0] as usize], w0);
                    }
                    if w1 != 0.0 {
                        dst[c_vert_value as usize]
                            .add_with_weight(&src[e_vert_values[1] as usize], w1);
                    }

                    // face weights
                    let nfw = active_mask.num_face_weights();
                    for f in 0..nfw.min(nef) {
                        let fw = active_mask.face_weight(f);
                        if fw == 0.0 {
                            continue;
                        }
                        let pface = efaces[f as i32];

                        if active_mask.face_weights_for_face_centers() {
                            // Use the already-computed face-child vertex value in dst
                            let c_vert_of_face = refinement.get_face_child_vertex(pface);
                            if c_vert_of_face != INDEX_INVALID {
                                let c_value_of_face =
                                    child_fvar.get_vertex_value_offset(c_vert_of_face, 0);
                                let face_val = dst[c_value_of_face as usize].clone();
                                dst[c_vert_value as usize].add_with_weight(&face_val, fw);
                            }
                        } else {
                            // Locate the edge within the face via getFaceEdges, then
                            // take (eInFace + 2) % faceSize as the opposite vertex index.
                            // Mirrors C++ primvarRefiner.h:790-801.
                            let p_face_edges = parent.get_face_edges(pface);
                            let p_face_values = parent_fvar.get_face_values(pface);
                            let fn_size = p_face_edges.size() as usize;
                            let mut e_in_face = 0usize;
                            for i in 0..fn_size {
                                if p_face_edges[i as i32] == pedge {
                                    e_in_face = i;
                                    break;
                                }
                            }
                            let v_in_face = (e_in_face + 2) % fn_size;
                            let p_value_next = p_face_values[v_in_face as i32];
                            dst[c_vert_value as usize]
                                .add_with_weight(&src[p_value_next as usize], fw);
                        }
                    }
                } else {
                    // --- Mismatch: each sibling gets linear 0.5/0.5 from its face ---
                    let n_siblings = c_vert_values.size() as usize;
                    for sib in 0..n_siblings {
                        // parent sibling source for this child sibling
                        let e_face_index =
                            refine_fvar.get_child_value_parent_source(cv, sib as i32);

                        let mut e_vert_values = [0i32; 2];
                        parent_fvar.get_edge_face_values(pedge, e_face_index, &mut e_vert_values);

                        let c_vert_value = c_vert_values[sib as i32];
                        dst[c_vert_value as usize].clear();
                        dst[c_vert_value as usize]
                            .add_with_weight(&src[e_vert_values[0] as usize], 0.5);
                        dst[c_vert_value as usize]
                            .add_with_weight(&src[e_vert_values[1] as usize], 0.5);
                    }
                }
            }
        }

        // ===================================================================
        // Phase 3 — interpFVarFromVerts (C++ 834-1002)
        // ===================================================================
        {
            let nv = refinement.get_num_child_vertices_from_vertices();
            let base = refinement.get_first_child_vertex_from_vertices();

            let mut edge_sharps = vec![0.0f32; max_v];
            let mut child_e_sharps = vec![0.0f32; max_v];
            let mut v_edge_values = vec![0i32; max_v];

            for i in 0..nv {
                let cv = base + i;
                let pvert = refinement.get_child_vertex_parent_index(cv);

                let p_vert_values = parent_fvar.get_vertex_values(pvert);
                let c_vert_values = child_fvar.get_vertex_values(cv);
                let matches_vertex = child_fvar.value_topology_matches(c_vert_values[0]);

                if is_linear_fvar && matches_vertex {
                    // Simple copy: dst[cVertValues[0]] = src[pVertValues[0]]
                    let p_vert_value = p_vert_values[0];
                    let c_vert_value = c_vert_values[0];
                    dst[c_vert_value as usize].clear();
                    dst[c_vert_value as usize].add_with_weight(&src[p_vert_value as usize], 1.0);
                    continue;
                }

                if matches_vertex {
                    // Full scheme mask computation
                    let vedges = parent.get_vertex_edges(pvert);
                    let vfaces = parent.get_vertex_faces(pvert);
                    let ne = vedges.size() as usize;
                    let nf = vfaces.size() as usize;
                    let vsharp = parent.get_vertex_sharpness(pvert);

                    edge_sharps.resize(ne, 0.0);
                    for k in 0..ne {
                        edge_sharps[k] = parent.get_edge_sharpness(vedges[k as i32]);
                    }
                    child_e_sharps.resize(ne, 0.0);
                    crease.subdivide_edge_sharpnesses_around_vertex(
                        &edge_sharps[..ne],
                        &mut child_e_sharps[..ne],
                    );
                    let child_vsharp = crease.subdivide_vertex_sharpness(vsharp);

                    let nbr = VertNbr {
                        sharpness: vsharp,
                        num_edges: ne,
                        num_faces: nf,
                        edge_sharp: edge_sharps[..ne].to_vec(),
                        child_sharp: child_vsharp,
                        child_e_sharp: child_e_sharps[..ne].to_vec(),
                    };

                    let mut mask = WeightMask::new(ne + 1);
                    mask.clear_all();
                    let p_rule = parent.get_vertex_rule(pvert);
                    let c_rule = child.get_vertex_rule(cv);
                    scheme.compute_vertex_vertex_mask(&nbr, &mut mask, p_rule, c_rule);

                    let p_vert_value = p_vert_values[0];
                    let c_vert_value = c_vert_values[0];
                    dst[c_vert_value as usize].clear();

                    // Face weights — use child fvar value offset for each face-child vertex
                    let nfw = mask.num_face_weights();
                    for k in 0..nf.min(nfw) {
                        let fw = mask.face_weight(k);
                        if fw == 0.0 {
                            continue;
                        }
                        let pface = vfaces[k as i32];

                        // C++ asserts AreFaceWeightsForFaceCenters() in interpFVarFromVerts
                        // (primvarRefiner.h:922). The else-branch is semantically wrong here.
                        debug_assert!(
                            mask.face_weights_for_face_centers(),
                            "fvar interp from verts: face weights must be for face centers"
                        );
                        let c_vert_of_face = refinement.get_face_child_vertex(pface);
                        if c_vert_of_face != INDEX_INVALID {
                            let c_value_of_face =
                                child_fvar.get_vertex_value_offset(c_vert_of_face, 0);
                            let face_val = dst[c_value_of_face as usize].clone();
                            dst[c_vert_value as usize].add_with_weight(&face_val, fw);
                        }
                    }

                    // Edge weights — use parentFVar.getVertexEdgeValues
                    let new = mask.num_edge_weights();
                    if new > 0 {
                        v_edge_values.resize(ne, 0);
                        parent_fvar.get_vertex_edge_values(pvert, &mut v_edge_values[..ne]);
                        for k in 0..ne.min(new) {
                            let ew = mask.edge_weight(k);
                            if ew == 0.0 {
                                continue;
                            }
                            dst[c_vert_value as usize]
                                .add_with_weight(&src[v_edge_values[k] as usize], ew);
                        }
                    }

                    // Vertex weight last (numerical precision, matches C++).
                    // C++ applies vVertWeight unconditionally (primvarRefiner.h:943).
                    debug_assert!(
                        mask.num_vertex_weights() > 0,
                        "fvar vertex mask must have a vertex weight"
                    );
                    let vw = mask.vertex_weight(0);
                    dst[c_vert_value as usize].add_with_weight(&src[p_vert_value as usize], vw);
                } else {
                    // Mismatch — each sibling is independently a corner or crease
                    let p_value_tags = parent_fvar.get_vertex_value_tags(pvert);
                    let c_value_tags = child_fvar.get_vertex_value_tags(cv);
                    let n_siblings = c_vert_values.size() as usize;

                    for c_sibling in 0..n_siblings {
                        // map child sibling → parent sibling
                        let p_sibling =
                            refine_fvar.get_child_value_parent_source(cv, c_sibling as i32);

                        let p_vert_value = p_vert_values[p_sibling as i32];
                        let c_vert_value = c_vert_values[c_sibling as i32];
                        dst[c_vert_value as usize].clear();

                        if is_linear_fvar || c_value_tags[c_sibling as i32].is_corner() {
                            // Corner or linear: simple copy
                            dst[c_vert_value as usize]
                                .add_with_weight(&src[p_vert_value as usize], 1.0);
                        } else {
                            // Crease (or semi-sharp transitioning to crease)
                            // base crease weights: 0.75 center + 0.125 each end
                            let mut v_weight = 0.75f32;
                            let mut e_weight = 0.125f32;

                            let p_tag = p_value_tags[p_sibling as i32];
                            if p_tag.is_semi_sharp() {
                                // Blend between crease and corner using fractional weight
                                let dep_sharp = p_tag.is_dep_sharp();
                                let w_corner = if dep_sharp {
                                    // depSharp: use opposite sibling for fractional weight
                                    let opp_p = if p_sibling == 0 { 1i32 } else { 0i32 };
                                    let opp_c = if c_sibling == 0 { 1i32 } else { 0i32 };
                                    refine_fvar.get_fractional_weight(
                                        pvert,
                                        opp_p as u16,
                                        cv,
                                        opp_c as u16,
                                    )
                                } else {
                                    refine_fvar.get_fractional_weight(
                                        pvert,
                                        p_sibling as u16,
                                        cv,
                                        c_sibling as u16,
                                    )
                                };
                                let w_crease = 1.0 - w_corner;
                                v_weight = w_crease * 0.75 + w_corner;
                                e_weight = w_crease * 0.125;
                            }

                            // Crease end values for this sibling
                            let mut end_values = [0i32; 2];
                            parent_fvar.get_vertex_crease_end_values(
                                pvert,
                                p_sibling as u16,
                                &mut end_values,
                            );

                            dst[c_vert_value as usize]
                                .add_with_weight(&src[end_values[0] as usize], e_weight);
                            dst[c_vert_value as usize]
                                .add_with_weight(&src[end_values[1] as usize], e_weight);
                            dst[c_vert_value as usize]
                                .add_with_weight(&src[p_vert_value as usize], v_weight);
                        }
                    }
                }
            }
        }
    }

    // -----------------------------------------------------------------------
    // Limit evaluation
    // -----------------------------------------------------------------------

    fn limit_impl<T, K>(
        &self,
        src: &[T],
        dst_pos: &mut [T],
        dst_tan1: Option<&mut [T]>,
        dst_tan2: Option<&mut [T]>,
    ) where
        T: Interpolatable,
        K: crate::sdc::scheme::SchemeKernel,
    {
        let max_level = self.refiner.get_max_level();
        let level = self.refiner.get_level_internal(max_level);
        let opts = self.scheme_options();
        let scheme = Scheme::<K>::with_options(opts);
        let crease = Crease::with_options(opts);
        let nv = level.get_num_vertices();
        let max_v = level.get_max_valence() as usize;

        let has_tangents = dst_tan1.is_some() && dst_tan2.is_some();
        let mut e_sh = vec![0.0f32; max_v];
        let mut ce_sh = vec![0.0f32; max_v];

        // Evaluate positions only (tangent path requires mutable re-borrowing,
        // handled separately to avoid lifetime complexity)
        for vert in 0..nv {
            let vsharp = level.get_vertex_sharpness(vert);
            let vedges = level.get_vertex_edges(vert);
            let vfaces = level.get_vertex_faces(vert);
            let ne = vedges.size() as usize;
            let nf = vfaces.size() as usize;

            e_sh.resize(ne, 0.0);
            for k in 0..ne {
                e_sh[k] = level.get_edge_sharpness(vedges[k as i32]);
            }
            ce_sh.resize(ne, 0.0);
            crease.subdivide_edge_sharpnesses_around_vertex(&e_sh[..ne], &mut ce_sh[..ne]);
            let cvsh = crease.subdivide_vertex_sharpness(vsharp);

            let nbr = VertNbr {
                sharpness: vsharp,
                num_edges: ne,
                num_faces: nf,
                edge_sharp: e_sh[..ne].to_vec(),
                child_sharp: cvsh,
                child_e_sharp: ce_sh[..ne].to_vec(),
            };

            let mut pos_mask = WeightMask::new(max_v);
            pos_mask.clear_all();
            scheme.compute_vertex_limit_mask(&nbr, &mut pos_mask, level.get_vertex_rule(vert));

            dst_pos[vert as usize].clear();
            let vw = if pos_mask.num_vertex_weights() > 0 {
                pos_mask.vertex_weight(0)
            } else {
                0.0
            };
            if vw != 0.0 {
                dst_pos[vert as usize].add_with_weight(&src[vert as usize], vw);
            }

            for k in 0..ne.min(pos_mask.num_edge_weights()) {
                let ew = pos_mask.edge_weight(k);
                if ew == 0.0 {
                    continue;
                }
                let e = vedges[k as i32];
                let ev = level.get_edge_vertices(e);
                let opp = if ev[0] == vert { ev[1] } else { ev[0] };
                dst_pos[vert as usize].add_with_weight(&src[opp as usize], ew);
            }

            for k in 0..nf.min(pos_mask.num_face_weights()) {
                let fw = pos_mask.face_weight(k);
                if fw == 0.0 {
                    continue;
                }
                let pface = vfaces[k as i32];
                let pfverts = level.get_face_vertices(pface);
                let opp = opp_vert_in_face_not(vert, pfverts.as_slice());
                if opp != INDEX_INVALID {
                    dst_pos[vert as usize].add_with_weight(&src[opp as usize], fw);
                }
            }
        }

        // Tangent evaluation — only if requested and no borrow conflict
        if has_tangents {
            if let (Some(t1), Some(t2)) = (dst_tan1, dst_tan2) {
                for vert in 0..nv {
                    t1[vert as usize].clear();
                    t2[vert as usize].clear();

                    let vsharp = level.get_vertex_sharpness(vert);
                    let vedges = level.get_vertex_edges(vert);
                    let vfaces = level.get_vertex_faces(vert);
                    let ne = vedges.size() as usize;
                    let nf = vfaces.size() as usize;

                    e_sh.resize(ne, 0.0);
                    for k in 0..ne {
                        e_sh[k] = level.get_edge_sharpness(vedges[k as i32]);
                    }
                    ce_sh.resize(ne, 0.0);
                    crease.subdivide_edge_sharpnesses_around_vertex(&e_sh[..ne], &mut ce_sh[..ne]);
                    let cvsh = crease.subdivide_vertex_sharpness(vsharp);

                    let nbr = VertNbr {
                        sharpness: vsharp,
                        num_edges: ne,
                        num_faces: nf,
                        edge_sharp: e_sh[..ne].to_vec(),
                        child_sharp: cvsh,
                        child_e_sharp: ce_sh[..ne].to_vec(),
                    };

                    let mut tm1 = WeightMask::new(max_v);
                    let mut tm2 = WeightMask::new(max_v);
                    let mut tm3 = WeightMask::new(max_v);
                    scheme.compute_vertex_limit_mask_with_tangents(
                        &nbr,
                        &mut tm1,
                        &mut tm2,
                        &mut tm3,
                        level.get_vertex_rule(vert),
                    );

                    // Gather face-neighbor indices for tangent computation.
                    // C++ uses fIndices[i] = face_verts[(vInFace+2) % faceSize]
                    // (diagonal vertex from the current vertex in each incident face).
                    let tan_vfaces = level.get_vertex_faces(vert);
                    let tan_vf_local = level.get_vertex_face_local_indices(vert);
                    let nf_tan = tan_vfaces.size() as usize;
                    let mut f_indices: Vec<Index> = Vec::with_capacity(nf_tan);
                    for fi in 0..nf_tan {
                        let pface = tan_vfaces[fi as i32];
                        let pfverts = level.get_face_vertices(pface);
                        let vin = tan_vf_local[fi as i32] as i32;
                        let mut opp = vin + 2;
                        if opp >= pfverts.size() as i32 {
                            opp -= pfverts.size() as i32;
                        }
                        f_indices.push(pfverts[opp]);
                    }

                    // Apply tan1 mask (first tangent — C++ dstTan1 uses tm2 weights)
                    t1[vert as usize].clear();
                    // face weights for tan1
                    for fi in 0..nf_tan.min(tm2.num_face_weights()) {
                        let fw = tm2.face_weight(fi);
                        if fw == 0.0 {
                            continue;
                        }
                        t1[vert as usize].add_with_weight(&src[f_indices[fi] as usize], fw);
                    }
                    // edge weights for tan1
                    for k in 0..ne.min(tm2.num_edge_weights()) {
                        let ew = tm2.edge_weight(k);
                        if ew == 0.0 {
                            continue;
                        }
                        let e = vedges[k as i32];
                        let ev = level.get_edge_vertices(e);
                        let opp = if ev[0] == vert { ev[1] } else { ev[0] };
                        t1[vert as usize].add_with_weight(&src[opp as usize], ew);
                    }
                    // vertex weight for tan1
                    let vw1 = if tm2.num_vertex_weights() > 0 {
                        tm2.vertex_weight(0)
                    } else {
                        0.0
                    };
                    if vw1 != 0.0 {
                        t1[vert as usize].add_with_weight(&src[vert as usize], vw1);
                    }

                    // Apply tan2 mask (second tangent — C++ dstTan2 uses tm3 weights)
                    t2[vert as usize].clear();
                    // face weights for tan2
                    for fi in 0..nf_tan.min(tm3.num_face_weights()) {
                        let fw = tm3.face_weight(fi);
                        if fw == 0.0 {
                            continue;
                        }
                        t2[vert as usize].add_with_weight(&src[f_indices[fi] as usize], fw);
                    }
                    // edge weights for tan2
                    for k in 0..ne.min(tm3.num_edge_weights()) {
                        let ew = tm3.edge_weight(k);
                        if ew == 0.0 {
                            continue;
                        }
                        let e = vedges[k as i32];
                        let ev = level.get_edge_vertices(e);
                        let opp = if ev[0] == vert { ev[1] } else { ev[0] };
                        t2[vert as usize].add_with_weight(&src[opp as usize], ew);
                    }
                    // vertex weight for tan2
                    let vw2 = if tm3.num_vertex_weights() > 0 {
                        tm3.vertex_weight(0)
                    } else {
                        0.0
                    };
                    if vw2 != 0.0 {
                        t2[vert as usize].add_with_weight(&src[vert as usize], vw2);
                    }
                } // for vert
            } // if let (Some(t1), Some(t2))
        } // if has_tangents
    }
}

// ---------------------------------------------------------------------------
// Utility functions
// ---------------------------------------------------------------------------

/// Find the vertex in `fverts` that is opposite the edge (e0, e1).
/// For a quad: the diagonal from either edge endpoint.
/// For a triangle: the vertex not on the edge.
fn opposite_vert_across_edge(e0: Index, e1: Index, fverts: &[Index]) -> Index {
    let n = fverts.len();
    for i in 0..n {
        let a = fverts[i];
        let b = fverts[(i + 1) % n];
        if (a == e0 && b == e1) || (a == e1 && b == e0) {
            // Edge found at position i→i+1; opposite = (i+2)%n for quads,
            // or (i+2)%n for triangles (the remaining vertex)
            return fverts[(i + 2) % n];
        }
    }
    INDEX_INVALID
}

/// Find a vertex in `fverts` that is not `pvert`.
/// Returns the "opposite" vertex for use in Loop face weights.
fn opp_vert_in_face_not(pvert: Index, fverts: &[Index]) -> Index {
    let n = fverts.len();
    match n {
        3 => {
            for i in 0..3 {
                if fverts[i] == pvert {
                    // Return the vertex diagonally opposite (for Loop: far corner)
                    return fverts[(i + 1) % 3];
                }
            }
        }
        4 => {
            for i in 0..4 {
                if fverts[i] == pvert {
                    return fverts[(i + 2) % 4];
                }
            }
        }
        _ => {}
    }
    INDEX_INVALID
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interpolatable_f32() {
        let mut v = 0.0f32;
        v.clear();
        v.add_with_weight(&2.0f32, 0.5);
        assert!((v - 1.0).abs() < 1e-6);
    }

    #[test]
    fn interpolatable_vec3() {
        let mut v = [0.0f32; 3];
        v.clear();
        v.add_with_weight(&[1.0, 2.0, 3.0], 0.5);
        assert!((v[0] - 0.5).abs() < 1e-6);
        assert!((v[1] - 1.0).abs() < 1e-6);
        assert!((v[2] - 1.5).abs() < 1e-6);
    }

    #[test]
    fn weight_mask() {
        let mut m = WeightMask::new(4);
        m.set_num_vertex_weights(2);
        m.set_vertex_weight(0, 0.5);
        m.set_vertex_weight(1, 0.5);
        assert!((m.vertex_weight(0) + m.vertex_weight(1) - 1.0).abs() < 1e-6);
    }
}
