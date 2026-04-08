//! HdSt Subdivision - OpenSubdiv refiner wrapper for Storm.
//!
//! Provides subdivision surface refinement using stencil tables and patch tables.
//! When the `subdivision` feature is enabled, uses opensubdiv-rs for real
//! Catmull-Clark / Loop / Bilinear subdivision via `Far::TopologyRefiner`
//! and `PrimvarRefiner`.
//!
//! Port of C++ `HdSt_Subdivision` (hdSt/subdivision.cpp).

use crate::mesh_topology::Interpolation;
use std::sync::{Arc, Mutex};
use usd_px_osd::SubdivTags;
use usd_tf::Token;

// ---------------------------------------------------------------------------
// Stencil / Patch table abstractions (kept for subdivision3.rs compat)
// ---------------------------------------------------------------------------

/// A stencil entry mapping one refined vertex to a weighted sum of coarse verts.
#[derive(Debug, Clone)]
pub struct StencilEntry {
    /// Coarse vertex indices
    pub indices: Vec<i32>,
    /// Corresponding weights (same length as indices)
    pub weights: Vec<f32>,
}

/// CPU stencil table: a collection of stencil entries for refinement.
#[derive(Debug, Clone, Default)]
pub struct StencilTable {
    /// One entry per refined vertex
    pub stencils: Vec<StencilEntry>,
}

impl StencilTable {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.stencils.len()
    }

    pub fn is_empty(&self) -> bool {
        self.stencils.is_empty()
    }

    /// Apply stencils to a primvar buffer (interleaved float components).
    pub fn apply(&self, src: &[f32], stride: usize) -> Vec<f32> {
        let mut dst = vec![0.0f32; self.stencils.len() * stride];
        for (si, stencil) in self.stencils.iter().enumerate() {
            let out_offset = si * stride;
            for (&idx, &w) in stencil.indices.iter().zip(&stencil.weights) {
                let in_offset = idx as usize * stride;
                // Guard against stencil indices that point beyond the source buffer.
                if in_offset + stride > src.len() {
                    continue;
                }
                for c in 0..stride {
                    dst[out_offset + c] += w * src[in_offset + c];
                }
            }
        }
        dst
    }
}

/// Patch type produced by subdivision.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PatchType {
    BSpline,
    Gregory,
    BoxSplineTriangle,
    Triangles,
    Quads,
}

/// A single patch descriptor.
#[derive(Debug, Clone)]
pub struct PatchDesc {
    pub patch_type: PatchType,
    pub cv_indices: Vec<i32>,
    pub param: u32,
}

/// Patch table: collection of patches produced by subdivision.
#[derive(Debug, Clone, Default)]
pub struct PatchTable {
    pub patches: Vec<PatchDesc>,
}

impl PatchTable {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn len(&self) -> usize {
        self.patches.len()
    }
    pub fn is_empty(&self) -> bool {
        self.patches.is_empty()
    }
}

/// GPU stencil table handle (opaque, backend-specific).
#[derive(Debug, Clone)]
pub struct GpuStencilTable {
    pub index_buffer_id: u64,
    pub weight_buffer_id: u64,
    pub num_stencils: usize,
}

pub type GpuStencilTableSharedPtr = Arc<GpuStencilTable>;

// ---------------------------------------------------------------------------
// HdStSubdivision (stencil-based, kept for compat)
// ---------------------------------------------------------------------------

/// Subdivision struct holding stencil/patch tables for CPU and GPU refinement.
pub struct HdStSubdivision {
    adaptive: bool,
    refine_level: i32,
    max_num_face_varying: i32,
    vertex_stencils: Option<StencilTable>,
    varying_stencils: Option<StencilTable>,
    face_varying_stencils: Vec<Option<StencilTable>>,
    patch_table: Option<PatchTable>,
    #[allow(dead_code)]
    gpu_stencil_mutex: Mutex<()>,
    #[allow(dead_code)]
    gpu_vertex_stencils: Option<GpuStencilTableSharedPtr>,
    #[allow(dead_code)]
    gpu_varying_stencils: Option<GpuStencilTableSharedPtr>,
    #[allow(dead_code)]
    gpu_face_varying_stencils: Vec<Option<GpuStencilTableSharedPtr>>,
}

impl std::fmt::Debug for HdStSubdivision {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HdStSubdivision")
            .field("adaptive", &self.adaptive)
            .field("refine_level", &self.refine_level)
            .finish()
    }
}

impl HdStSubdivision {
    pub fn new(adaptive: bool, refine_level: i32) -> Self {
        Self {
            adaptive,
            refine_level,
            max_num_face_varying: 0,
            vertex_stencils: None,
            varying_stencils: None,
            face_varying_stencils: Vec::new(),
            patch_table: None,
            gpu_stencil_mutex: Mutex::new(()),
            gpu_vertex_stencils: None,
            gpu_varying_stencils: None,
            gpu_face_varying_stencils: Vec::new(),
        }
    }

    pub fn is_adaptive(&self) -> bool {
        self.adaptive
    }
    pub fn get_refine_level(&self) -> i32 {
        self.refine_level
    }
    pub fn get_num_vertices(&self) -> usize {
        self.vertex_stencils.as_ref().map_or(0, |s| s.len())
    }
    pub fn get_num_varying(&self) -> usize {
        self.varying_stencils.as_ref().map_or(0, |s| s.len())
    }
    pub fn get_num_face_varying(&self, channel: usize) -> usize {
        self.face_varying_stencils
            .get(channel)
            .and_then(|s| s.as_ref())
            .map_or(0, |s| s.len())
    }
    pub fn get_max_num_face_varying(&self) -> i32 {
        self.max_num_face_varying
    }

    pub fn set_refinement_tables(
        &mut self,
        vertex_stencils: Option<StencilTable>,
        varying_stencils: Option<StencilTable>,
        face_varying_stencils: Vec<Option<StencilTable>>,
        patch_table: Option<PatchTable>,
    ) {
        self.max_num_face_varying = face_varying_stencils.len() as i32;
        self.vertex_stencils = vertex_stencils;
        self.varying_stencils = varying_stencils;
        self.face_varying_stencils = face_varying_stencils;
        self.patch_table = patch_table;
    }

    pub fn get_stencil_table(
        &self,
        interpolation: Interpolation,
        fvar_channel: usize,
    ) -> Option<&StencilTable> {
        match interpolation {
            Interpolation::Vertex => self.vertex_stencils.as_ref(),
            Interpolation::Varying => self.varying_stencils.as_ref(),
            Interpolation::FaceVarying => self
                .face_varying_stencils
                .get(fvar_channel)
                .and_then(|s| s.as_ref()),
        }
    }

    pub fn get_patch_table(&self) -> Option<&PatchTable> {
        self.patch_table.as_ref()
    }

    pub fn refine_cpu(
        &self,
        source: &[f32],
        stride: usize,
        interpolation: Interpolation,
        fvar_channel: usize,
    ) -> Vec<f32> {
        match self.get_stencil_table(interpolation, fvar_channel) {
            Some(t) => t.apply(source, stride),
            None => Vec::new(),
        }
    }

    pub fn scheme_refines_to_triangles(scheme: &Token) -> bool {
        scheme == "loop"
    }
    pub fn scheme_refines_to_bspline_patches(scheme: &Token) -> bool {
        scheme == "catmullClark"
    }
    pub fn scheme_refines_to_box_spline_triangle_patches(scheme: &Token) -> bool {
        scheme == "loop"
    }
}

pub type HdStSubdivisionSharedPtr = Arc<HdStSubdivision>;

// ===========================================================================
// OpenSubdiv-backed subdivision (feature = "subdivision")
// ===========================================================================

/// Result of subdivision refinement.
#[derive(Debug, Clone, Default)]
pub struct SubdivResult {
    /// Refined vertex positions (3 floats per vertex).
    pub positions: Vec<f32>,
    /// Refined vertex normals (3 floats per vertex, may be empty).
    pub normals: Vec<f32>,
    /// Refined face-vertex counts (all quads for Catmark, all tris for Loop).
    pub face_vertex_counts: Vec<i32>,
    /// Refined face-vertex indices (flattened).
    pub face_vertex_indices: Vec<i32>,
    /// Number of refined vertices.
    pub num_vertices: i32,
}

/// Refine a mesh using opensubdiv-rs uniform subdivision.
///
/// Returns `None` when `level == 0`, topology is empty, or refiner creation fails.
///
/// * `scheme` - "catmullClark", "loop", or "bilinear"
/// * `face_vertex_counts` - per-face vertex counts
/// * `face_vertex_indices` - flattened face-vertex indices
/// * `hole_indices` - face indices tagged as holes (skipped during refinement)
/// * `subdiv_tags` - crease/corner/interpolation data from USD prim
/// * `positions` - flat f32 positions (3 per vertex)
/// * `normals` - flat f32 normals (3 per vertex, may be empty)
/// * `level` - uniform refinement level (1..=8)
#[cfg(feature = "subdivision")]
pub fn subdivide_mesh(
    scheme: &str,
    face_vertex_counts: &[i32],
    face_vertex_indices: &[i32],
    hole_indices: &[i32],
    subdiv_tags: &SubdivTags,
    positions: &[f32],
    normals: &[f32],
    level: u32,
) -> Option<SubdivResult> {
    use opensubdiv_rs::far::primvar_refiner::PrimvarRefiner;
    use opensubdiv_rs::far::{
        FactoryOptions, TopologyDescriptor, TopologyDescriptorFactory, TopologyRefinerFactory,
        UniformOptions,
    };
    use opensubdiv_rs::sdc::{
        CreasingMethod as OsdCreasingMethod, FVarLinearInterpolation as OsdFVarLinearInterpolation,
        Options as SdcOptions, SchemeType, VtxBoundaryInterpolation as OsdVtxBoundaryInterpolation,
    };
    use usd_px_osd::{CreasingMethod, FVarLinearInterpolation, VtxBoundaryInterpolation};

    // P1-4: Validate position buffer length
    if positions.len() % 3 != 0 || positions.is_empty() {
        return None;
    }

    if level == 0 || face_vertex_counts.is_empty() {
        return None;
    }

    let num_verts = (positions.len() / 3) as i32;

    let scheme_type = match scheme {
        "catmullClark" | "catmark" => SchemeType::Catmark,
        "loop" => SchemeType::Loop,
        "bilinear" => SchemeType::Bilinear,
        _ => SchemeType::Catmark,
    };

    // P0-1: Loop scheme requires all-triangular faces
    if scheme_type == SchemeType::Loop {
        if face_vertex_counts.iter().any(|&c| c != 3) {
            log::warn!("Cannot apply Loop subdivision: non-triangular faces");
            return None;
        }
    }

    // --- Bug 1 fix: crease tags ---
    // Expand crease chains into per-edge pairs. Each chain of length N has N-1 edges.
    // crease_weights may be per-chain or per-edge; we replicate per-chain weights to per-edge.
    let (edge_crease_indices, edge_crease_weights) = {
        let indices = subdiv_tags.crease_indices();
        let lengths = subdiv_tags.crease_lengths();
        let weights = subdiv_tags.crease_weights();

        let mut pairs: Vec<i32> = Vec::new();
        let mut edge_weights: Vec<f32> = Vec::new();
        let per_chain_weights = weights.len() == lengths.len();

        let mut offset = 0usize;
        for (ci, &len) in lengths.iter().enumerate() {
            let len = len as usize;
            // Extract (len - 1) consecutive edge pairs from this crease chain
            for i in 0..len.saturating_sub(1) {
                if offset + i + 1 < indices.len() {
                    pairs.push(indices[offset + i]);
                    pairs.push(indices[offset + i + 1]);
                    // Use per-chain or per-edge weight
                    let w = if per_chain_weights {
                        weights.get(ci).copied().unwrap_or(0.0)
                    } else {
                        weights.get(offset + i).copied().unwrap_or(0.0)
                    };
                    edge_weights.push(w);
                }
            }
            offset += len;
        }
        (pairs, edge_weights)
    };

    let num_creases = (edge_crease_indices.len() / 2) as i32;
    let corner_indices: Vec<i32> = subdiv_tags.corner_indices().to_vec();
    let corner_weights: Vec<f32> = subdiv_tags.corner_weights().to_vec();

    // Build OSD topology descriptor with crease/corner/hole data
    let desc = TopologyDescriptor {
        num_vertices: num_verts,
        num_faces: face_vertex_counts.len() as i32,
        num_verts_per_face: face_vertex_counts.to_vec(),
        vert_indices_per_face: face_vertex_indices.to_vec(),
        // Crease data (edge pairs + sharpness)
        num_creases,
        crease_vertex_index_pairs: edge_crease_indices,
        crease_weights: edge_crease_weights,
        // Corner (sharp vertex) data
        num_corners: corner_indices.len() as i32,
        corner_vertex_indices: corner_indices,
        corner_weights,
        // Holes: faces tagged as holes are skipped by OSD refiner
        hole_indices: hole_indices.to_vec(),
        ..TopologyDescriptor::default()
    };

    // --- Bug 2 fix: SdcOptions from subdiv tags (typed enum accessors) ---
    let mut sdc_opts = SdcOptions::new();

    // Map vertex boundary interpolation; bilinear scheme always uses EdgeAndCorner.
    let vtx_interp = if scheme_type == SchemeType::Bilinear {
        OsdVtxBoundaryInterpolation::EdgeAndCorner
    } else {
        match subdiv_tags.get_vtx_boundary_interpolation() {
            VtxBoundaryInterpolation::None => OsdVtxBoundaryInterpolation::None,
            VtxBoundaryInterpolation::EdgeOnly => OsdVtxBoundaryInterpolation::EdgeOnly,
            VtxBoundaryInterpolation::EdgeAndCorner => OsdVtxBoundaryInterpolation::EdgeAndCorner,
        }
    };
    sdc_opts.set_vtx_boundary_interpolation(vtx_interp);

    // Map face-varying linear interpolation via typed enum.
    let fvar_interp = match subdiv_tags.get_fvar_linear_interpolation() {
        FVarLinearInterpolation::None => OsdFVarLinearInterpolation::None,
        FVarLinearInterpolation::CornersOnly => OsdFVarLinearInterpolation::CornersOnly,
        FVarLinearInterpolation::CornersPlus1 => OsdFVarLinearInterpolation::CornersPlus1,
        FVarLinearInterpolation::CornersPlus2 => OsdFVarLinearInterpolation::CornersPlus2,
        FVarLinearInterpolation::Boundaries => OsdFVarLinearInterpolation::Boundaries,
        FVarLinearInterpolation::All => OsdFVarLinearInterpolation::All,
    };
    sdc_opts.set_fvar_linear_interpolation(fvar_interp);

    // Map crease method via typed enum.
    let crease_method = match subdiv_tags.get_crease_method() {
        CreasingMethod::Uniform => OsdCreasingMethod::Uniform,
        CreasingMethod::Chaikin => OsdCreasingMethod::Chaikin,
    };
    sdc_opts.set_creasing_method(crease_method);

    let factory_opts = FactoryOptions::new(scheme_type, sdc_opts);
    let mut refiner = TopologyDescriptorFactory::create(&desc, factory_opts)?;

    // Uniform refinement
    let uni_opts = UniformOptions::new(level.min(8));
    refiner.refine_uniform(uni_opts);

    let pr = PrimvarRefiner::new(&refiner);
    let num_levels = refiner.get_num_levels();
    if num_levels <= 1 {
        return None;
    }

    // Reinterpret flat positions as &[[f32; 3]]
    let base_pos = to_vec3_slice(positions);

    // Interpolate positions through each refinement level
    let mut pos_src: Vec<[f32; 3]> = base_pos.to_vec();
    for lvl in 1..num_levels {
        let n_dst = refiner.get_level(lvl).get_num_vertices() as usize;
        let mut pos_dst = vec![[0.0f32; 3]; n_dst];
        pr.interpolate(lvl, &pos_src, &mut pos_dst);
        pos_src = pos_dst;
    }

    // Interpolate normals if provided
    let mut normals_out = Vec::new();
    // P0-2/3: Only interpolate normals if count matches vertex count
    if normals.len() / 3 >= num_verts as usize {
        let base_nrm = to_vec3_slice(normals);
        let mut nrm_src: Vec<[f32; 3]> = base_nrm.to_vec();
        for lvl in 1..num_levels {
            let n_dst = refiner.get_level(lvl).get_num_vertices() as usize;
            let mut nrm_dst = vec![[0.0f32; 3]; n_dst];
            pr.interpolate(lvl, &nrm_src, &mut nrm_dst);
            nrm_src = nrm_dst;
        }
        // Re-normalize
        normals_out.reserve(nrm_src.len() * 3);
        for n in &nrm_src {
            let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
            if len > 1e-8 {
                normals_out.extend_from_slice(&[n[0] / len, n[1] / len, n[2] / len]);
            } else {
                normals_out.extend_from_slice(&[0.0, 0.0, 1.0]);
            }
        }
    }

    // Extract refined topology from the finest level
    let finest = refiner.get_level(num_levels - 1);
    let num_faces = finest.get_num_faces();
    let refined_num_verts = finest.get_num_vertices();

    let mut fvc = Vec::with_capacity(num_faces as usize);
    let mut fvi = Vec::new();
    for f in 0..num_faces {
        let face_verts = finest.get_face_vertices(f);
        let n = face_verts.size();
        fvc.push(n);
        for i in 0..n {
            fvi.push(face_verts[i]);
        }
    }

    let positions_flat: Vec<f32> = pos_src.iter().flat_map(|p| [p[0], p[1], p[2]]).collect();

    Some(SubdivResult {
        positions: positions_flat,
        normals: normals_out,
        face_vertex_counts: fvc,
        face_vertex_indices: fvi,
        num_vertices: refined_num_verts,
    })
}

/// Stub when `subdivision` feature is disabled.
#[cfg(not(feature = "subdivision"))]
pub fn subdivide_mesh(
    _scheme: &str,
    _face_vertex_counts: &[i32],
    _face_vertex_indices: &[i32],
    _hole_indices: &[i32],
    _subdiv_tags: &SubdivTags,
    _positions: &[f32],
    _normals: &[f32],
    _level: u32,
) -> Option<SubdivResult> {
    use std::sync::atomic::{AtomicBool, Ordering};
    static WARNED: AtomicBool = AtomicBool::new(false);
    if !WARNED.swap(true, Ordering::Relaxed) {
        log::info!(
            "subdivide_mesh: `subdivision` feature is disabled, meshes will not be subdivided"
        );
    }
    None
}

/// Reinterpret `&[f32]` as `&[[f32; 3]]` (zero-copy).
#[cfg(feature = "subdivision")]
fn to_vec3_slice(data: &[f32]) -> &[[f32; 3]] {
    let count = data.len() / 3;
    // Safety: [f32; 3] has same layout as 3 contiguous f32s, no padding.
    unsafe { std::slice::from_raw_parts(data.as_ptr() as *const [f32; 3], count) }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stencil_apply() {
        let table = StencilTable {
            stencils: vec![StencilEntry {
                indices: vec![0, 1],
                weights: vec![0.5, 0.5],
            }],
        };
        let src = vec![0.0, 0.0, 0.0, 2.0, 0.0, 0.0, 0.0, 2.0, 0.0];
        let dst = table.apply(&src, 3);
        assert_eq!(dst.len(), 3);
        assert!((dst[0] - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_subdivision_new() {
        let subdiv = HdStSubdivision::new(false, 2);
        assert!(!subdiv.is_adaptive());
        assert_eq!(subdiv.get_refine_level(), 2);
        assert_eq!(subdiv.get_num_vertices(), 0);
    }

    #[test]
    fn test_scheme_queries() {
        assert!(HdStSubdivision::scheme_refines_to_triangles(&Token::new(
            "loop"
        )));
        assert!(!HdStSubdivision::scheme_refines_to_triangles(&Token::new(
            "catmullClark"
        )));
    }

    #[test]
    fn test_refine_cpu() {
        let mut subdiv = HdStSubdivision::new(false, 1);
        let table = StencilTable {
            stencils: vec![
                StencilEntry {
                    indices: vec![0],
                    weights: vec![1.0],
                },
                StencilEntry {
                    indices: vec![0, 1],
                    weights: vec![0.5, 0.5],
                },
                StencilEntry {
                    indices: vec![1],
                    weights: vec![1.0],
                },
            ],
        };
        subdiv.set_refinement_tables(Some(table), None, vec![], None);
        let src = vec![0.0, 0.0, 0.0, 4.0, 0.0, 0.0];
        let dst = subdiv.refine_cpu(&src, 3, Interpolation::Vertex, 0);
        assert_eq!(dst.len(), 9);
        assert!((dst[3] - 2.0).abs() < 1e-6);
    }

    // --- OpenSubdiv-backed tests (only run with `subdivision` feature) ---

    #[cfg(feature = "subdivision")]
    #[test]
    fn catmark_quad_level1() {
        let fvc = [4];
        let fvi = [0, 1, 2, 3];
        let positions: Vec<f32> = vec![
            0.0, 0.0, 0.0, // v0
            1.0, 0.0, 0.0, // v1
            1.0, 1.0, 0.0, // v2
            0.0, 1.0, 0.0, // v3
        ];
        let tags = SubdivTags::default();

        let r = subdivide_mesh("catmullClark", &fvc, &fvi, &[], &tags, &positions, &[], 1);
        assert!(r.is_some(), "subdivision should succeed");
        let r = r.unwrap();

        // Catmull-Clark level 1: 4 corners + 4 edge mids + 1 face center = 9
        assert_eq!(r.num_vertices, 9, "expected 9 vertices at level 1");
        assert_eq!(r.face_vertex_counts.len(), 4, "expected 4 sub-faces");
        assert!(r.face_vertex_counts.iter().all(|&c| c == 4), "all quads");
        assert_eq!(r.face_vertex_indices.len(), 16);
        assert_eq!(r.positions.len(), 27); // 9 * 3

        // Face center at (0.5, 0.5, 0) should exist
        let center_found = (0..9).any(|i| {
            let x = r.positions[i * 3];
            let y = r.positions[i * 3 + 1];
            let z = r.positions[i * 3 + 2];
            (x - 0.5).abs() < 1e-4 && (y - 0.5).abs() < 1e-4 && z.abs() < 1e-4
        });
        assert!(center_found, "face center at (0.5, 0.5, 0) expected");
    }

    #[cfg(feature = "subdivision")]
    #[test]
    fn level_zero_returns_none() {
        let fvc = [4];
        let fvi = [0, 1, 2, 3];
        let pos = vec![0.0; 12];
        let tags = SubdivTags::default();
        assert!(subdivide_mesh("catmullClark", &fvc, &fvi, &[], &tags, &pos, &[], 0).is_none());
    }

    #[cfg(feature = "subdivision")]
    #[test]
    fn catmark_quad_level2() {
        let fvc = [4];
        let fvi = [0, 1, 2, 3];
        let positions: Vec<f32> = vec![0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 1.0, 0.0, 0.0, 1.0, 0.0];
        let tags = SubdivTags::default();

        let r1 =
            subdivide_mesh("catmullClark", &fvc, &fvi, &[], &tags, &positions, &[], 1).unwrap();
        let r2 =
            subdivide_mesh("catmullClark", &fvc, &fvi, &[], &tags, &positions, &[], 2).unwrap();
        assert!(r2.num_vertices > r1.num_vertices);
        assert!(r2.face_vertex_counts.len() > r1.face_vertex_counts.len());
    }

    #[cfg(feature = "subdivision")]
    #[test]
    fn normals_interpolated() {
        let fvc = [4];
        let fvi = [0, 1, 2, 3];
        let positions: Vec<f32> = vec![0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 1.0, 0.0, 0.0, 1.0, 0.0];
        let normals: Vec<f32> = vec![0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0];
        let tags = SubdivTags::default();

        let r = subdivide_mesh(
            "catmullClark",
            &fvc,
            &fvi,
            &[],
            &tags,
            &positions,
            &normals,
            1,
        )
        .unwrap();
        assert_eq!(r.normals.len(), r.num_vertices as usize * 3);
        // All normals should remain ~(0,0,1) since base is uniform
        for i in 0..r.num_vertices as usize {
            let nz = r.normals[i * 3 + 2];
            assert!(nz > 0.99, "vertex {} normal Z = {}", i, nz);
        }
    }

    #[cfg(feature = "subdivision")]
    #[test]
    fn crease_tags_populated() {
        use usd_tf::Token;
        // Single edge crease: chain [0,1], length=2, weight=2.0
        let tags = SubdivTags::new(
            Token::new("edgeAndCorner"),
            Token::new("boundaries"),
            Token::new("uniform"),
            Token::default(),
            vec![0, 1], // crease_indices: one chain, verts 0→1
            vec![2],    // crease_lengths: chain of 2
            vec![2.0],  // crease_weights: sharpness 2.0
            vec![],     // no corners
            vec![],
        );
        let fvc = [4];
        let fvi = [0, 1, 2, 3];
        let positions: Vec<f32> = vec![0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 1.0, 0.0, 0.0, 1.0, 0.0];
        // Should succeed — crease tags fed to OSD
        let r = subdivide_mesh("catmullClark", &fvc, &fvi, &[], &tags, &positions, &[], 1);
        assert!(r.is_some(), "subdivision with creases should succeed");
    }
}
