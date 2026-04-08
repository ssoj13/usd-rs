// Copyright 2014 DreamWorks Animation LLC.
// Ported to Rust from OpenSubdiv 3.7.0 sdc/scheme.h
//
// Design notes
// -------------
// C++ uses a class template Scheme<SCHEME_TYPE> with template methods that
// accept arbitrary FACE/EDGE/VERTEX/MASK types.  In Rust we model this with:
//
//   * `MaskInterface` -- the required capability of a mask storage object
//   * `FaceNeighborhood` / `EdgeNeighborhood` / `VertexNeighborhood` -- the
//     topology query traits
//   * `SchemeImpl` -- a struct that holds `Options` and provides all mask
//     computation methods; concrete implementations in bilinear/catmark/loop
//     modules implement the `SchemeKernel` trait which supplies the
//     scheme-specific weight assignments
//   * `Scheme<K>` -- thin wrapper over `SchemeImpl` parameterised by the kernel

use smallvec::SmallVec;

use super::crease::{Crease, Rule};
use super::options::Options;

/// Inline stack capacity for LocalMask weight buffers.
///
/// C++ uses `alloca()` — we use SmallVec with a fixed inline size that covers
/// typical vertex valences (regular = 4 or 6) without heap allocation.
/// 20 covers all but pathologically high-valence vertices in production meshes.
const LOCAL_MASK_INLINE: usize = 20;

// ─────────────────────────────────────────────────────────────────────────────
// Weight type alias -- mirrors `MASK::Weight` in C++.  Callers may use f32 or
// f64; we pick f32 as the standard concrete weight type.
// ─────────────────────────────────────────────────────────────────────────────

pub type Weight = f32;

// ─────────────────────────────────────────────────────────────────────────────
// MaskInterface -- writable weight buffer with counts.
// Mirrors the public interface required of the MASK template parameter.
// ─────────────────────────────────────────────────────────────────────────────

/// Required interface for a mask weight buffer.
///
/// Any struct that stores vertex/edge/face weights for a subdivision mask must
/// implement this trait.  The canonical implementation is `WeightMask`.
pub trait MaskInterface {
    // -- count accessors -------------------------------------------------------
    fn num_vertex_weights(&self) -> usize;
    fn num_edge_weights(&self) -> usize;
    fn num_face_weights(&self) -> usize;

    fn set_num_vertex_weights(&mut self, n: usize);
    fn set_num_edge_weights(&mut self, n: usize);
    fn set_num_face_weights(&mut self, n: usize);

    // -- weight accessors (immutable) ------------------------------------------
    fn vertex_weight(&self, i: usize) -> Weight;
    fn edge_weight(&self, i: usize) -> Weight;
    fn face_weight(&self, i: usize) -> Weight;

    // -- weight accessors (mutable) --------------------------------------------
    fn set_vertex_weight(&mut self, i: usize, w: Weight);
    fn set_edge_weight(&mut self, i: usize, w: Weight);
    fn set_face_weight(&mut self, i: usize, w: Weight);

    // -- face-weight interpretation flag --------------------------------------
    /// True when face weights represent face-centre points (Catmark); false
    /// when they represent opposite vertices (Loop).
    fn face_weights_for_face_centers(&self) -> bool;
    fn set_face_weights_for_face_centers(&mut self, v: bool);
}

// ─────────────────────────────────────────────────────────────────────────────
// Neighbourhood query traits (mirrors FACE / EDGE / VERTEX template params)
// ─────────────────────────────────────────────────────────────────────────────

/// Topology information about the face neighbourhood for a face-vertex.
pub trait FaceNeighborhood {
    /// Number of vertices (corner count) of this face.
    fn num_vertices(&self) -> usize;
}

/// Topology and sharpness info for an edge-vertex mask query.
pub trait EdgeNeighborhood {
    /// Number of adjacent faces.
    fn num_faces(&self) -> usize;
    /// Sharpness of the edge itself.
    fn sharpness(&self) -> f32;
    /// Number of vertices in each adjacent face (for triangle detection in
    /// Catmark with TRI_SUB_SMOOTH).
    fn num_vertices_per_face(&self, counts: &mut [usize]);
    /// Compute and fill child edge sharpnesses using the given Crease.
    fn child_sharpnesses(&self, crease: &Crease, out: &mut [f32; 2]);
}

/// Topology and sharpness info for a vertex-vertex mask query.
pub trait VertexNeighborhood {
    /// Number of incident edges (= number of incident faces for interior manifold).
    fn num_edges(&self) -> usize;
    /// Number of incident faces.
    fn num_faces(&self) -> usize;
    /// Vertex sharpness.
    fn sharpness(&self) -> f32;
    /// Fill `out` with per-edge sharpness values (length = `num_edges()`).
    fn sharpness_per_edge<'a>(&self, out: &'a mut [f32]) -> &'a [f32];
    /// Child vertex sharpness (subdivide the vertex sharpness).
    fn child_sharpness(&self, crease: &Crease) -> f32;
    /// Fill `out` with per-edge child sharpness values.
    fn child_sharpness_per_edge<'a>(&self, crease: &Crease, out: &'a mut [f32]) -> &'a [f32];
}

// ─────────────────────────────────────────────────────────────────────────────
// SchemeKernel -- scheme-specific weight assignments
// ─────────────────────────────────────────────────────────────────────────────

/// Scheme-specific weight assignment methods.
///
/// Implementors supply the inner logic for each mask type; the generic
/// `compute_*` methods in `Scheme<K>` handle all creasing/Rule logic and then
/// delegate to these.
///
/// Two optional override hooks let schemes bypass the generic crease/Rule logic
/// entirely.  They mirror the C++ full specialisations of `ComputeEdgeVertexMask`
/// and `ComputeVertexVertexMask` for the Bilinear scheme, which ignore all
/// sharpness and directly call the crease/corner assign functions.
pub trait SchemeKernel {
    // Static trait info
    fn topological_split_type() -> super::types::Split;
    fn regular_face_size() -> i32;
    fn regular_vertex_valence() -> i32;
    fn local_neighborhood_size() -> i32;

    // ── Optional full-override hooks ─────────────────────────────────────────
    //
    // These mirror the C++ full specialisations of `ComputeEdgeVertexMask` and
    // `ComputeVertexVertexMask`.  Return `true` if the mask was set (i.e. the
    // generic crease/Rule logic should be skipped), `false` to fall through to
    // the base implementation.
    //
    // Default: return false (use the generic base logic).  Bilinear overrides
    // to `true` because it ignores ALL sharpness values — its edge-vertex mask
    // is always the midpoint and its vertex-vertex mask is always identity.

    /// If this returns `true`, `compute_edge_vertex_mask` stops immediately.
    ///
    /// C++ bilinearScheme.h specialises `ComputeEdgeVertexMask` to call
    /// `assignCreaseMaskForEdge` directly, discarding `parentRule`/`childRule`
    /// and sharpness entirely.  This hook replicates that behaviour.
    #[inline]
    fn override_compute_edge_vertex_mask<E: EdgeNeighborhood, M: MaskInterface>(
        _options: &Options,
        _edge: &E,
        _mask: &mut M,
        _p_rule: Rule,
        _c_rule: Rule,
    ) -> bool {
        false
    }

    /// If this returns `true`, `compute_vertex_vertex_mask` stops immediately.
    ///
    /// C++ bilinearScheme.h specialises `ComputeVertexVertexMask` to call
    /// `assignCornerMaskForVertex` directly, discarding all sharpness/Rule
    /// complexity.  This hook replicates that behaviour.
    #[inline]
    fn override_compute_vertex_vertex_mask<V: VertexNeighborhood, M: MaskInterface>(
        _options: &Options,
        _vertex: &V,
        _mask: &mut M,
        _p_rule: Rule,
        _c_rule: Rule,
    ) -> bool {
        false
    }

    // Edge-vertex masks
    fn assign_crease_mask_for_edge<E: EdgeNeighborhood, M: MaskInterface>(
        options: &Options,
        edge: &E,
        mask: &mut M,
    );
    fn assign_smooth_mask_for_edge<E: EdgeNeighborhood, M: MaskInterface>(
        options: &Options,
        edge: &E,
        mask: &mut M,
    );

    // Vertex-vertex masks
    fn assign_corner_mask_for_vertex<V: VertexNeighborhood, M: MaskInterface>(
        options: &Options,
        vertex: &V,
        mask: &mut M,
    );
    fn assign_crease_mask_for_vertex<V: VertexNeighborhood, M: MaskInterface>(
        options: &Options,
        vertex: &V,
        mask: &mut M,
        crease_ends: [usize; 2],
    );
    fn assign_smooth_mask_for_vertex<V: VertexNeighborhood, M: MaskInterface>(
        options: &Options,
        vertex: &V,
        mask: &mut M,
    );

    // Limit masks -- position
    fn assign_corner_limit_mask<V: VertexNeighborhood, M: MaskInterface>(
        options: &Options,
        vertex: &V,
        mask: &mut M,
    );
    fn assign_crease_limit_mask<V: VertexNeighborhood, M: MaskInterface>(
        options: &Options,
        vertex: &V,
        mask: &mut M,
        crease_ends: [usize; 2],
    );
    fn assign_smooth_limit_mask<V: VertexNeighborhood, M: MaskInterface>(
        options: &Options,
        vertex: &V,
        mask: &mut M,
    );

    // Limit masks -- tangents
    fn assign_corner_limit_tangent_masks<V: VertexNeighborhood, M: MaskInterface>(
        options: &Options,
        vertex: &V,
        tan1: &mut M,
        tan2: &mut M,
    );
    fn assign_crease_limit_tangent_masks<V: VertexNeighborhood, M: MaskInterface>(
        options: &Options,
        vertex: &V,
        tan1: &mut M,
        tan2: &mut M,
        crease_ends: [usize; 2],
    );
    fn assign_smooth_limit_tangent_masks<V: VertexNeighborhood, M: MaskInterface>(
        options: &Options,
        vertex: &V,
        tan1: &mut M,
        tan2: &mut M,
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// LocalMask -- internal scratch mask for combining two masks during transitions
// Mirrors C++ Scheme<SCHEME>::LocalMask<WEIGHT>
// ─────────────────────────────────────────────────────────────────────────────

/// Stack-optimised local mask used when blending two rules at a transition.
///
/// C++ uses `alloca()` for the weight arrays — we use `SmallVec` with a
/// `LOCAL_MASK_INLINE` inline capacity so the common case (valence ≤ 20) never
/// touches the heap, matching C++'s stack-allocation strategy.
struct LocalMask {
    v_weights: SmallVec<[Weight; 1]>,
    e_weights: SmallVec<[Weight; LOCAL_MASK_INLINE]>,
    f_weights: SmallVec<[Weight; LOCAL_MASK_INLINE]>,
    v_count: usize,
    e_count: usize,
    f_count: usize,
    f_weights_for_centers: bool,
}

impl LocalMask {
    fn new(valence: usize) -> Self {
        let mut e = SmallVec::new();
        e.resize(valence, 0.0);
        let mut f = SmallVec::new();
        f.resize(valence, 0.0);
        Self {
            v_weights: smallvec::smallvec![0.0; 1],
            e_weights: e,
            f_weights: f,
            v_count: 0,
            e_count: 0,
            f_count: 0,
            f_weights_for_centers: false,
        }
    }

    /// Blend this (child) mask into `dst` (parent) mask using:
    ///   dst = this_coeff * self + dst_coeff * dst
    ///
    /// Mirrors C++ `LocalMask::CombineVertexVertexMasks`.
    fn combine_into<M: MaskInterface>(&self, this_coeff: Weight, dst_coeff: Weight, dst: &mut M) {
        // Vertex weight (always exactly 1 for vertex-vertex masks)
        let v = dst_coeff * dst.vertex_weight(0) + this_coeff * self.v_weights[0];
        dst.set_vertex_weight(0, v);

        // Edge weights (child may have more than parent)
        let edge_count = self.e_count;
        if edge_count > 0 {
            if dst.num_edge_weights() == 0 {
                dst.set_num_edge_weights(edge_count);
                for i in 0..edge_count {
                    dst.set_edge_weight(i, this_coeff * self.e_weights[i]);
                }
            } else {
                for i in 0..edge_count {
                    let w = dst_coeff * dst.edge_weight(i) + this_coeff * self.e_weights[i];
                    dst.set_edge_weight(i, w);
                }
            }
        }

        // Face weights (child may have more than parent)
        let face_count = self.f_count;
        if face_count > 0 {
            if dst.num_face_weights() == 0 {
                dst.set_num_face_weights(face_count);
                dst.set_face_weights_for_face_centers(self.f_weights_for_centers);
                for i in 0..face_count {
                    dst.set_face_weight(i, this_coeff * self.f_weights[i]);
                }
            } else {
                // Both have face weights -- their interpretation must agree
                debug_assert_eq!(
                    self.f_weights_for_centers,
                    dst.face_weights_for_face_centers()
                );
                for i in 0..face_count {
                    let w = dst_coeff * dst.face_weight(i) + this_coeff * self.f_weights[i];
                    dst.set_face_weight(i, w);
                }
            }
        }
    }
}

impl MaskInterface for LocalMask {
    fn num_vertex_weights(&self) -> usize {
        self.v_count
    }
    fn num_edge_weights(&self) -> usize {
        self.e_count
    }
    fn num_face_weights(&self) -> usize {
        self.f_count
    }
    fn set_num_vertex_weights(&mut self, n: usize) {
        self.v_count = n;
    }
    fn set_num_edge_weights(&mut self, n: usize) {
        self.e_count = n;
    }
    fn set_num_face_weights(&mut self, n: usize) {
        self.f_count = n;
    }
    fn vertex_weight(&self, i: usize) -> Weight {
        self.v_weights[i]
    }
    fn edge_weight(&self, i: usize) -> Weight {
        self.e_weights[i]
    }
    fn face_weight(&self, i: usize) -> Weight {
        self.f_weights[i]
    }
    fn set_vertex_weight(&mut self, i: usize, w: Weight) {
        self.v_weights[i] = w;
    }
    fn set_edge_weight(&mut self, i: usize, w: Weight) {
        self.e_weights[i] = w;
    }
    fn set_face_weight(&mut self, i: usize, w: Weight) {
        self.f_weights[i] = w;
    }
    fn face_weights_for_face_centers(&self) -> bool {
        self.f_weights_for_centers
    }
    fn set_face_weights_for_face_centers(&mut self, v: bool) {
        self.f_weights_for_centers = v;
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Scheme<K> -- main entry point, generic over the kernel
// ─────────────────────────────────────────────────────────────────────────────

/// Subdivision scheme parameterised by a kernel `K: SchemeKernel`.
///
/// Mirrors the C++ `Sdc::Scheme<SCHEME_TYPE>` class template.
pub struct Scheme<K: SchemeKernel> {
    options: Options,
    _kernel: std::marker::PhantomData<K>,
}

impl<K: SchemeKernel> Scheme<K> {
    pub fn new() -> Self {
        Self {
            options: Options::default(),
            _kernel: std::marker::PhantomData,
        }
    }
    pub fn with_options(options: Options) -> Self {
        Self {
            options,
            _kernel: std::marker::PhantomData,
        }
    }

    pub fn options(&self) -> Options {
        self.options
    }
    pub fn set_options(&mut self, o: Options) {
        self.options = o;
    }

    // Static trait queries (forwarded to kernel)
    pub fn topological_split_type() -> super::types::Split {
        K::topological_split_type()
    }
    pub fn regular_face_size() -> i32 {
        K::regular_face_size()
    }
    pub fn regular_vertex_valence() -> i32 {
        K::regular_vertex_valence()
    }
    pub fn local_neighborhood_size() -> i32 {
        K::local_neighborhood_size()
    }

    // ── Face-vertex mask ──────────────────────────────────────────────────────

    /// Compute the face-vertex mask: uniform 1/N over all N corner vertices.
    /// Same for all schemes.
    pub fn compute_face_vertex_mask<F, M>(&self, face: &F, mask: &mut M)
    where
        F: FaceNeighborhood,
        M: MaskInterface,
    {
        let n = face.num_vertices();
        mask.set_num_vertex_weights(n);
        mask.set_num_edge_weights(0);
        mask.set_num_face_weights(0);
        mask.set_face_weights_for_face_centers(false);

        let w = 1.0 / n as Weight;
        for i in 0..n {
            mask.set_vertex_weight(i, w);
        }
    }

    // ── Edge-vertex mask ──────────────────────────────────────────────────────

    /// Compute the edge-vertex mask, handling smooth/crease/transitional cases.
    ///
    /// Optional `parent_rule` / `child_rule` accelerate the computation when
    /// already known.  Pass `Rule::Unknown` for automatic determination.
    ///
    /// If the kernel overrides `override_compute_edge_vertex_mask` (returning
    /// `true`), the generic crease/sharpness logic is skipped entirely.  This
    /// matches the C++ full specialisation of `ComputeEdgeVertexMask` for the
    /// Bilinear scheme, which directly assigns the crease midpoint mask.
    pub fn compute_edge_vertex_mask<E, M>(
        &self,
        edge: &E,
        mask: &mut M,
        parent_rule: Rule,
        child_rule: Rule,
    ) where
        E: EdgeNeighborhood,
        M: MaskInterface,
    {
        // S2 fix: allow scheme to bypass all crease/sharpness logic entirely.
        // Bilinear overrides this to assign the crease midpoint mask directly,
        // matching C++ `Scheme<SCHEME_BILINEAR>::ComputeEdgeVertexMask`.
        if K::override_compute_edge_vertex_mask(&self.options, edge, mask, parent_rule, child_rule)
        {
            return;
        }

        // ── Smooth parent: return smooth mask immediately ──────────────────
        if parent_rule == Rule::Smooth || (parent_rule == Rule::Unknown && edge.sharpness() <= 0.0)
        {
            K::assign_smooth_mask_for_edge(&self.options, edge, mask);
            return;
        }

        // ── Child known to be crease: return crease mask immediately ──────
        if child_rule == Rule::Crease {
            K::assign_crease_mask_for_edge(&self.options, edge, mask);
            return;
        }

        // ── Parent is crease, child unknown or smooth ─────────────────────
        let crease = Crease::with_options(self.options);
        let child_is_crease = if child_rule == Rule::Unknown {
            // Determine child rule from sharpness
            if parent_rule == Rule::Crease {
                true
            } else if edge.sharpness() >= 1.0 {
                // Sharpness >= 1 always produces a crease child (fractional
                // weight >= 1.0 clamped to 1.0 = full crease)
                true
            } else if crease.is_uniform() {
                // Uniform: sharpness < 1.0 always decays to 0
                false
            } else {
                // Chaikin: check if both child edges remain sharp
                let mut c_sharp = [0.0f32; 2];
                edge.child_sharpnesses(&crease, &mut c_sharp);
                c_sharp[0] > 0.0 && c_sharp[1] > 0.0
            }
        } else {
            child_rule == Rule::Crease
        };

        if child_is_crease {
            K::assign_crease_mask_for_edge(&self.options, edge, mask);
            return;
        }

        // ── Crease-to-Smooth transition ───────────────────────────────────
        // Compute smooth mask for child, then linearly blend in the crease
        // midpoint contribution weighted by parent sharpness.
        K::assign_smooth_mask_for_edge(&self.options, edge, mask);

        let p_weight = edge.sharpness();
        let c_weight = 1.0 - p_weight;

        let v0 = p_weight * 0.5 + c_weight * mask.vertex_weight(0);
        let v1 = p_weight * 0.5 + c_weight * mask.vertex_weight(1);
        mask.set_vertex_weight(0, v0);
        mask.set_vertex_weight(1, v1);

        let fc = mask.num_face_weights();
        for i in 0..fc {
            let fw = mask.face_weight(i) * c_weight;
            mask.set_face_weight(i, fw);
        }
    }

    // ── Vertex-vertex mask ────────────────────────────────────────────────────

    /// Compute the vertex-vertex mask, handling all rule combinations including
    /// smooth/dart, corner, crease, and transitions.
    ///
    /// Pass `Rule::Unknown` for `parent_rule` to auto-detect; if `parent_rule`
    /// is known but `child_rule` is not, pass `Rule::Unknown` for `child_rule`
    /// to indicate "same as parent" (no transition).
    ///
    /// If the kernel overrides `override_compute_vertex_vertex_mask` (returning
    /// `true`), the generic crease/sharpness logic is skipped entirely.  This
    /// matches the C++ full specialisation for the Bilinear scheme, which
    /// directly assigns the corner (identity) mask.
    pub fn compute_vertex_vertex_mask<V, M>(
        &self,
        vertex: &V,
        mask: &mut M,
        p_rule: Rule,
        c_rule: Rule,
    ) where
        V: VertexNeighborhood,
        M: MaskInterface,
    {
        // S2 fix: allow scheme to bypass all crease/sharpness logic entirely.
        // Bilinear overrides this to assign the corner identity mask directly,
        // matching C++ `Scheme<SCHEME_BILINEAR>::ComputeVertexVertexMask`.
        if K::override_compute_vertex_vertex_mask(&self.options, vertex, mask, p_rule, c_rule) {
            return;
        }

        self.compute_vertex_vertex_mask_generic(vertex, mask, p_rule, c_rule);
    }

    /// Generic vertex-vertex mask logic (crease/Rule aware).
    /// Separated so bilinear's override hook can skip it entirely.
    fn compute_vertex_vertex_mask_generic<V, M>(
        &self,
        vertex: &V,
        mask: &mut M,
        mut p_rule: Rule,
        mut c_rule: Rule,
    ) where
        V: VertexNeighborhood,
        M: MaskInterface,
    {
        // Quick path: smooth / dart vertex
        if p_rule == Rule::Smooth || p_rule == Rule::Dart {
            K::assign_smooth_mask_for_vertex(&self.options, vertex, mask);
            return;
        }
        // If parent known but child not, assume same rule (no transition)
        if c_rule == Rule::Unknown && p_rule != Rule::Unknown {
            c_rule = p_rule;
        }

        let valence = vertex.num_edges();
        let mut p_edge_buf = vec![0.0f32; valence];
        let p_vertex_sharpness;

        // Determine whether we need parent sharpness
        let need_parent = p_rule == Rule::Unknown || p_rule == Rule::Crease || p_rule != c_rule;

        let p_edge_sharpness: &[f32];

        if need_parent {
            p_vertex_sharpness = vertex.sharpness();
            p_edge_sharpness = vertex.sharpness_per_edge(&mut p_edge_buf);
            if p_rule == Rule::Unknown {
                p_rule = Crease::with_options(self.options)
                    .determine_vertex_vertex_rule(p_vertex_sharpness, p_edge_sharpness);
            }
        } else {
            p_vertex_sharpness = 0.0;
            p_edge_sharpness = &p_edge_buf;
        }

        if p_rule == Rule::Smooth || p_rule == Rule::Dart {
            K::assign_smooth_mask_for_vertex(&self.options, vertex, mask);
            return;
        } else if p_rule == Rule::Crease {
            let crease_ends =
                Crease::with_options(self.options).get_sharp_edge_pair_of_crease(p_edge_sharpness);
            K::assign_crease_mask_for_vertex(&self.options, vertex, mask, crease_ends);
        } else {
            // Corner
            K::assign_corner_mask_for_vertex(&self.options, vertex, mask);
        }

        if c_rule == p_rule {
            return;
        }

        // ── Transition: compute child mask and blend ───────────────────────
        let crease = Crease::with_options(self.options);
        let mut c_edge_buf = vec![0.0f32; valence];
        let c_edge_sharpness = vertex.child_sharpness_per_edge(&crease, &mut c_edge_buf);
        let c_vertex_sharpness = vertex.child_sharpness(&crease);

        if c_rule == Rule::Unknown {
            c_rule = crease.determine_vertex_vertex_rule(c_vertex_sharpness, c_edge_sharpness);
            if c_rule == p_rule {
                return;
            }
        }

        // Build local child mask
        let mut c_mask = LocalMask::new(valence);

        if c_rule == Rule::Smooth || c_rule == Rule::Dart {
            K::assign_smooth_mask_for_vertex(&self.options, vertex, &mut c_mask);
        } else if c_rule == Rule::Crease {
            let c_ends = crease.get_sharp_edge_pair_of_crease(c_edge_sharpness);
            K::assign_crease_mask_for_vertex(&self.options, vertex, &mut c_mask, c_ends);
        } else {
            K::assign_corner_mask_for_vertex(&self.options, vertex, &mut c_mask);
        }

        let p_weight = crease.compute_fractional_weight_at_vertex(
            p_vertex_sharpness,
            c_vertex_sharpness,
            p_edge_sharpness,
            Some(c_edge_sharpness),
        );
        let c_weight = 1.0 - p_weight;

        c_mask.combine_into(c_weight, p_weight, mask);
    }

    // ── Limit position mask ───────────────────────────────────────────────────

    /// Compute the limit-position mask for a vertex.
    ///
    /// `rule` must not be `Unknown` -- it specifies the rule at the last
    /// refinement level.
    ///
    /// C++ name: `Scheme::ComputeVertexLimitMask` (position-only overload).
    #[doc(alias = "ComputeVertexLimitMask")]
    pub fn compute_vertex_limit_mask<V, M>(&self, vertex: &V, mask: &mut M, rule: Rule)
    where
        V: VertexNeighborhood,
        M: MaskInterface,
    {
        if rule == Rule::Smooth || rule == Rule::Dart {
            K::assign_smooth_limit_mask(&self.options, vertex, mask);
        } else if rule == Rule::Crease {
            let mut buf = vec![0.0f32; vertex.num_edges()];
            vertex.sharpness_per_edge(&mut buf);
            let ends = Crease::with_options(self.options).get_sharp_edge_pair_of_crease(&buf);
            K::assign_crease_limit_mask(&self.options, vertex, mask, ends);
        } else {
            K::assign_corner_limit_mask(&self.options, vertex, mask);
        }
    }

    /// Compute the limit-position and two tangent masks simultaneously.
    ///
    /// C++ name: `Scheme::ComputeVertexLimitMask` (position+tangents overload).
    /// Rust uses a distinct name since Rust does not support function overloading.
    #[doc(alias = "ComputeVertexLimitMask")]
    pub fn compute_vertex_limit_mask_with_tangents<V, M>(
        &self,
        vertex: &V,
        pos_mask: &mut M,
        tan1: &mut M,
        tan2: &mut M,
        rule: Rule,
    ) where
        V: VertexNeighborhood,
        M: MaskInterface,
    {
        if rule == Rule::Smooth || rule == Rule::Dart {
            K::assign_smooth_limit_mask(&self.options, vertex, pos_mask);
            K::assign_smooth_limit_tangent_masks(&self.options, vertex, tan1, tan2);
        } else if rule == Rule::Crease {
            let mut buf = vec![0.0f32; vertex.num_edges()];
            vertex.sharpness_per_edge(&mut buf);
            let ends = Crease::with_options(self.options).get_sharp_edge_pair_of_crease(&buf);
            K::assign_crease_limit_mask(&self.options, vertex, pos_mask, ends);
            K::assign_crease_limit_tangent_masks(&self.options, vertex, tan1, tan2, ends);
        } else {
            K::assign_corner_limit_mask(&self.options, vertex, pos_mask);
            K::assign_corner_limit_tangent_masks(&self.options, vertex, tan1, tan2);
        }
    }
}

impl<K: SchemeKernel> Default for Scheme<K> {
    fn default() -> Self {
        Self::new()
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// WeightMask -- canonical heap-allocated MaskInterface implementation
// ─────────────────────────────────────────────────────────────────────────────

/// A heap-allocated, dynamically-sized mask weight buffer.
///
/// Use this as the concrete `M` type when calling `Scheme` methods.
#[derive(Debug, Clone)]
pub struct WeightMask {
    pub v: Vec<Weight>,
    pub e: Vec<Weight>,
    pub f: Vec<Weight>,
    pub v_count: usize,
    pub e_count: usize,
    pub f_count: usize,
    pub f_for_centers: bool,
}

impl WeightMask {
    /// Allocate a mask with capacity for `max_v` vertex, `max_e` edge, and
    /// `max_f` face weights.
    pub fn new(max_v: usize, max_e: usize, max_f: usize) -> Self {
        Self {
            v: vec![0.0; max_v],
            e: vec![0.0; max_e],
            f: vec![0.0; max_f],
            v_count: 0,
            e_count: 0,
            f_count: 0,
            f_for_centers: false,
        }
    }
}

impl MaskInterface for WeightMask {
    fn num_vertex_weights(&self) -> usize {
        self.v_count
    }
    fn num_edge_weights(&self) -> usize {
        self.e_count
    }
    fn num_face_weights(&self) -> usize {
        self.f_count
    }
    fn set_num_vertex_weights(&mut self, n: usize) {
        self.v_count = n;
    }
    fn set_num_edge_weights(&mut self, n: usize) {
        self.e_count = n;
    }
    fn set_num_face_weights(&mut self, n: usize) {
        self.f_count = n;
    }
    fn vertex_weight(&self, i: usize) -> Weight {
        self.v[i]
    }
    fn edge_weight(&self, i: usize) -> Weight {
        self.e[i]
    }
    fn face_weight(&self, i: usize) -> Weight {
        self.f[i]
    }
    fn set_vertex_weight(&mut self, i: usize, w: Weight) {
        self.v[i] = w;
    }
    fn set_edge_weight(&mut self, i: usize, w: Weight) {
        self.e[i] = w;
    }
    fn set_face_weight(&mut self, i: usize, w: Weight) {
        self.f[i] = w;
    }
    fn face_weights_for_face_centers(&self) -> bool {
        self.f_for_centers
    }
    fn set_face_weights_for_face_centers(&mut self, v: bool) {
        self.f_for_centers = v;
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Common kernel helpers: crease and corner masks identical across all schemes
// ─────────────────────────────────────────────────────────────────────────────

/// Assign the crease mask for an edge-vertex: midpoint (0.5, 0.5).
/// Same for all schemes -- mirrors `Scheme<SCHEME>::assignCreaseMaskForEdge`.
pub fn assign_crease_mask_for_edge_common<E: EdgeNeighborhood, M: MaskInterface>(
    _edge: &E,
    mask: &mut M,
) {
    mask.set_num_vertex_weights(2);
    mask.set_num_edge_weights(0);
    mask.set_num_face_weights(0);
    mask.set_face_weights_for_face_centers(false);
    mask.set_vertex_weight(0, 0.5);
    mask.set_vertex_weight(1, 0.5);
}

/// Assign the corner mask for a vertex-vertex: identity weight 1.0.
/// Same for all schemes -- mirrors `Scheme<SCHEME>::assignCornerMaskForVertex`.
pub fn assign_corner_mask_for_vertex_common<V: VertexNeighborhood, M: MaskInterface>(
    _vertex: &V,
    mask: &mut M,
) {
    mask.set_num_vertex_weights(1);
    mask.set_num_edge_weights(0);
    mask.set_num_face_weights(0);
    mask.set_face_weights_for_face_centers(false);
    mask.set_vertex_weight(0, 1.0);
}

#[cfg(test)]
mod tests {
    use super::*;

    // Simple face with N vertices
    struct SimpleFace(usize);
    impl FaceNeighborhood for SimpleFace {
        fn num_vertices(&self) -> usize {
            self.0
        }
    }

    #[test]
    fn face_vertex_mask_quad() {
        use crate::sdc::bilinear_scheme::BilinearKernel;
        let scheme = Scheme::<BilinearKernel>::new();
        let face = SimpleFace(4);
        let mut mask = WeightMask::new(4, 0, 0);
        scheme.compute_face_vertex_mask(&face, &mut mask);

        assert_eq!(mask.num_vertex_weights(), 4);
        for i in 0..4 {
            assert!(
                (mask.vertex_weight(i) - 0.25).abs() < 1e-6,
                "weight[{}] = {}",
                i,
                mask.vertex_weight(i)
            );
        }
    }

    #[test]
    fn face_vertex_mask_tri() {
        use crate::sdc::loop_scheme::LoopKernel;
        let scheme = Scheme::<LoopKernel>::new();
        let face = SimpleFace(3);
        let mut mask = WeightMask::new(3, 0, 0);
        scheme.compute_face_vertex_mask(&face, &mut mask);

        assert_eq!(mask.num_vertex_weights(), 3);
        for i in 0..3 {
            assert!((mask.vertex_weight(i) - 1.0 / 3.0).abs() < 1e-6);
        }
    }
}
