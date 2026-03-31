
//! HdStMesh - Storm mesh prim implementation.
//!
//! Implements mesh rendering for the Storm backend including vertex/index
//! buffer synchronization and draw item management.

use crate::buffer_resource::{HdStBufferArrayRange, HdStBufferResourceSharedPtr};
use crate::draw_item::{
    HdBufferArrayRangeSharedPtr, HdStDrawItem, HdStDrawItemSharedPtr, MaterialTextureHandles,
    TopologyToPrimvarEntry, TopologyToPrimvarVector,
};
use crate::ext_comp_gpu_computation::get_ext_computation_primvars_computations;
use crate::resource_registry::{
    BufferArrayUsageHint, BufferSource, BufferSourceSharedPtr, BufferSpec, HdStResourceRegistry,
    ManagedBarSharedPtr,
};
use crate::wgsl_code_gen::MaterialParams;
use std::cell::RefCell;
use std::sync::{Arc, OnceLock};
use usd_gf;
use usd_hd::change_tracker::HdRprimDirtyBits;
use usd_hd::enums::HdInterpolation;
use usd_hd::prim::HdSceneDelegate;
use usd_hd::scene_delegate::HdPrimvarDescriptor;
use usd_hd::types::HdDirtyBits;
use usd_px_osd::SubdivTags;

// Thread-local aggregate timing for sync_from_delegate across all meshes in one frame.
thread_local! {
    static SYNC_STATS: RefCell<SyncStats> = RefCell::new(SyncStats::default());
}

#[derive(Default)]
struct SyncStats {
    count: u32,
    delegate_ms: f64,
    topo_ms: f64,
    vert_ms: f64,
    commit_ms: f64,
    // Sub-delegate breakdown
    vis_ms: f64,
    topo_read_ms: f64,
    xform_read_ms: f64,
    points_read_ms: f64,
    normals_read_ms: f64,
    other_pv_ms: f64,
    primvar_desc_ms: f64,
    fvar_meta_ms: f64,
    ext_comp_ms: f64,
}

impl SyncStats {
    fn reset(&mut self) {
        *self = Self::default();
    }
}
use usd_sdf::Path as SdfPath;
use usd_tf::Token;

/// Storm mesh-specific dirty bits, extending HdRprim dirty bits.
///
/// Port of C++ HdStMesh::DirtyBits (mesh.h). These extend the base HdRprimDirtyBits
/// with Storm-specific invalidation flags for computed data.
pub mod dirty_bits {
    /// Dirty smooth normals computed on GPU/CPU.
    pub const DIRTY_SMOOTH_NORMALS: u32 = 0x0100_0000; // bit 24 = CustomBitsBegin
    /// Dirty flat normals computed from topology.
    pub const DIRTY_FLAT_NORMALS: u32 = 0x0200_0000; // bit 25
    /// Dirty triangle index buffer.
    pub const DIRTY_INDICES: u32 = 0x0400_0000; // bit 26
    /// Dirty hull (control cage) index buffer.
    pub const DIRTY_HULL_INDICES: u32 = 0x0800_0000; // bit 27
    /// Dirty point cloud index buffer.
    pub const DIRTY_POINTS_INDICES: u32 = 0x1000_0000; // bit 28
}

// ============================================================================
// Mesh repr descriptors (port of C++ _MeshReprConfig)
// ============================================================================

/// Geometry style for a mesh repr descriptor.
///
/// Port of C++ HdMeshGeomStyle enum used in _MeshReprConfig::DescArray.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MeshGeomStyle {
    /// Invalid / skip this descriptor
    Invalid,
    /// Smooth hull surface (triangulated, smooth normals)
    SmoothHull,
    /// Flat hull surface (triangulated, flat normals)
    FlatHull,
    /// Hull (control cage) edge-only wireframe
    HullEdgeOnly,
    /// Hull (control cage) edge on surface
    HullEdgeOnSurf,
    /// Full hull surface
    Hull,
    /// Refined (subdivision) surface
    Refined,
    /// Points only
    Points,
}

/// Describes how a single draw item within a repr should be configured.
///
/// Port of C++ HdMeshReprDesc (one element of _MeshReprConfig::DescArray).
pub struct MeshReprDesc {
    /// Geometry drawing style
    pub geom_style: MeshGeomStyle,
    /// Whether this draw item uses flat shading (flat normals)
    pub flat_shading: bool,
}

/// Get repr descriptors for a given repr token.
///
/// Port of C++ HdStMesh::_GetReprDesc. Maps repr tokens to an array of
/// MeshReprDesc that defines which draw items to create.
///
/// Standard repr tokens and their descriptors:
/// - "smoothHull" / "refined" / "" / "default": 1 smooth surface item
/// - "hull": 1 flat-shaded hull item
/// - "wireOnSurf": 2 items (smooth surface + hull wireframe overlay)
/// - "refinedWireOnSurf": 2 items (refined surface + hull wireframe overlay)
/// - "wire": 1 hull edge-only wireframe item
/// - "points": 1 points item
pub fn get_mesh_repr_descs(repr_token: &Token) -> Vec<MeshReprDesc> {
    match repr_token.as_str() {
        // SmoothHull: single smooth-shaded triangulated surface
        "smoothHull" | "refined" | "" | "default" => vec![MeshReprDesc {
            geom_style: MeshGeomStyle::SmoothHull,
            flat_shading: false,
        }],
        // Hull: flat-shaded control cage
        "hull" => vec![MeshReprDesc {
            geom_style: MeshGeomStyle::Hull,
            flat_shading: true,
        }],
        // WireOnSurf: smooth surface + wireframe overlay
        "wireOnSurf" => vec![
            MeshReprDesc {
                geom_style: MeshGeomStyle::SmoothHull,
                flat_shading: false,
            },
            MeshReprDesc {
                geom_style: MeshGeomStyle::HullEdgeOnSurf,
                flat_shading: true,
            },
        ],
        // RefinedWireOnSurf: refined surface + wireframe overlay
        "refinedWireOnSurf" => vec![
            MeshReprDesc {
                geom_style: MeshGeomStyle::Refined,
                flat_shading: false,
            },
            MeshReprDesc {
                geom_style: MeshGeomStyle::HullEdgeOnSurf,
                flat_shading: true,
            },
        ],
        // Wire: edge-only wireframe
        "wire" | "refinedWire" => vec![MeshReprDesc {
            geom_style: MeshGeomStyle::HullEdgeOnly,
            flat_shading: true,
        }],
        // Points: point cloud
        "points" => vec![MeshReprDesc {
            geom_style: MeshGeomStyle::Points,
            flat_shading: false,
        }],
        // Unknown repr: default to smoothHull
        _ => {
            log::warn!(
                "HdStMesh: unknown repr '{}', falling back to smoothHull",
                repr_token.as_str()
            );
            vec![MeshReprDesc {
                geom_style: MeshGeomStyle::SmoothHull,
                flat_shading: false,
            }]
        }
    }
}

/// Mesh topology data for triangulation and buffer sync.
#[derive(Debug, Clone, Default)]
pub struct HdStMeshTopology {
    /// Number of vertices per face (e.g., [4, 3, 4] for quad, tri, quad)
    pub face_vertex_counts: Vec<i32>,
    /// Vertex indices for all faces flattened
    pub face_vertex_indices: Vec<i32>,
    /// Indices of hole faces to skip
    pub hole_indices: Vec<i32>,
    /// Orientation (true = right-handed)
    pub right_handed: bool,
    /// Subdivision scheme (e.g. "none", "catmullClark", "loop")
    pub subdivision_scheme: Token,
    /// Subdivision surface tags (creases, corners, interpolation rules).
    pub subdiv_tags: SubdivTags,
    /// Explicit vertex (point) count, set from the USD points array length.
    ///
    /// Mirrors C++ HdMeshTopology which stores numPoints explicitly.
    /// 0 = unknown (not yet set from delegate / points primvar).
    pub num_points: usize,
}

impl HdStMeshTopology {
    /// Create empty topology.
    pub fn new() -> Self {
        Self {
            face_vertex_counts: Vec::new(),
            face_vertex_indices: Vec::new(),
            hole_indices: Vec::new(),
            right_handed: true,
            subdivision_scheme: Token::new("none"),
            subdiv_tags: SubdivTags::default(),
            num_points: 0,
        }
    }

    /// Create from face data.
    pub fn from_faces(counts: Vec<i32>, indices: Vec<i32>) -> Self {
        Self {
            face_vertex_counts: counts,
            face_vertex_indices: indices,
            hole_indices: Vec::new(),
            right_handed: true,
            subdivision_scheme: Token::new("none"),
            subdiv_tags: SubdivTags::default(),
            num_points: 0,
        }
    }

    /// Get total number of faces.
    pub fn get_face_count(&self) -> usize {
        self.face_vertex_counts.len()
    }

    /// Get total number of vertices (points).
    ///
    /// Returns the explicitly stored `num_points` when set (preferred), matching
    /// C++ HdMeshTopology which stores numPoints directly from the USD points array.
    /// Falls back to max(indices)+1 scan only when num_points is 0 (not yet populated).
    pub fn get_vertex_count(&self) -> usize {
        if self.num_points > 0 {
            return self.num_points;
        }
        // Fallback: derive from indices (O(n), imprecise for non-contiguous indices).
        self.face_vertex_indices
            .iter()
            .map(|&i| i as usize)
            .max()
            .map(|m| m + 1)
            .unwrap_or(0)
    }

    /// Set the explicit vertex count from the USD points array.
    ///
    /// Call after loading the points primvar. Matches C++ HdMeshTopology
    /// constructor which takes numPoints directly.
    pub fn set_num_points(&mut self, n: usize) {
        self.num_points = n;
    }

    /// Triangulate the mesh, returning triangle indices.
    ///
    /// Converts polygons to triangles using fan triangulation.
    /// Returns (triangle_indices, triangle_count).
    pub fn triangulate(&self) -> (Vec<u32>, usize) {
        // Pre-calculate output size to avoid reallocation
        let hole_mask = self.build_hole_mask();
        let total_tris: usize = self
            .face_vertex_counts
            .iter()
            .enumerate()
            .filter(|(fi, _)| !hole_mask.as_ref().is_some_and(|m| *fi < m.len() && m[*fi]))
            .map(|(_, &c)| (c as usize).saturating_sub(2))
            .sum();
        let mut triangles = Vec::with_capacity(total_tris * 3);
        let mut index_offset = 0usize;

        for (face_idx, &count) in self.face_vertex_counts.iter().enumerate() {
            // Skip holes
            if hole_mask
                .as_ref()
                .is_some_and(|mask| face_idx < mask.len() && mask[face_idx])
            {
                index_offset += count as usize;
                continue;
            }

            let count = count as usize;
            if count < 3 {
                index_offset += count;
                continue;
            }

            // Bounds check: skip degenerate faces where vertex data is missing.
            if index_offset + count > self.face_vertex_indices.len() {
                break;
            }

            // Fan triangulation: first vertex is the hub
            let first_idx = self.face_vertex_indices[index_offset] as u32;

            for i in 1..count - 1 {
                let idx1 = self.face_vertex_indices[index_offset + i] as u32;
                let idx2 = self.face_vertex_indices[index_offset + i + 1] as u32;

                if self.right_handed {
                    triangles.push(first_idx);
                    triangles.push(idx1);
                    triangles.push(idx2);
                } else {
                    // Flip winding for left-handed
                    triangles.push(first_idx);
                    triangles.push(idx2);
                    triangles.push(idx1);
                }
            }

            index_offset += count;
        }

        let tri_count = triangles.len() / 3;

        // Trace: triangulation output diagnostics
        let max_idx = triangles.iter().copied().max().unwrap_or(0);
        let idx_count = self.face_vertex_indices.len();
        log::trace!(
            "HdStMesh::triangulate: faces={} idx_in={} tri_out={} max_vtx_idx={} first3={:?} last3={:?}",
            self.face_vertex_counts.len(),
            idx_count,
            tri_count,
            max_idx,
            &triangles[..triangles.len().min(3)],
            &triangles[triangles.len().saturating_sub(3)..]
        );

        (triangles, tri_count)
    }

    /// Fan-triangulate a faceVarying primvar (f32 tuples).
    ///
    /// Port of HdMeshUtil::ComputeTriangulatedFaceVaryingPrimvar.
    /// Input:  M values with `components` floats each (one per face-vertex).
    /// Output: numTris * 3 values (fan-triangulated to match triangle draw order).
    pub fn triangulate_fv(&self, src: &[f32], components: usize) -> Vec<f32> {
        let hole_mask = self.build_hole_mask();
        let total_tris: usize = self
            .face_vertex_counts
            .iter()
            .enumerate()
            .filter(|(fi, _)| !hole_mask.as_ref().is_some_and(|m| *fi < m.len() && m[*fi]))
            .map(|(_, &c)| (c as usize).saturating_sub(2))
            .sum();
        let mut dst = Vec::with_capacity(total_tris * 3 * components);
        let mut offset = 0usize;

        for (face_idx, &count) in self.face_vertex_counts.iter().enumerate() {
            let n = count as usize;
            if hole_mask
                .as_ref()
                .is_some_and(|mask| face_idx < mask.len() && mask[face_idx])
            {
                offset += n;
                continue;
            }
            if n < 3 {
                offset += n;
                continue;
            }
            if (offset + n) * components > src.len() {
                break;
            }
            // Fan triangulation: v0 is hub
            for i in 1..n - 1 {
                let i0 = offset;
                let (i1, i2) = if self.right_handed {
                    (offset + i, offset + i + 1)
                } else {
                    (offset + i + 1, offset + i)
                };
                dst.extend_from_slice(&src[i0 * components..(i0 + 1) * components]);
                dst.extend_from_slice(&src[i1 * components..(i1 + 1) * components]);
                dst.extend_from_slice(&src[i2 * components..(i2 + 1) * components]);
            }
            offset += n;
        }
        dst
    }

    /// Expand per-vertex data to per-face-vertex via faceVertexIndices, then
    /// fan-triangulate.
    ///
    /// Input: N unique vertex values (f32 tuples with `components` floats).
    /// Output: numTris * 3 values (each face-vertex gets its vertex's data).
    pub fn expand_and_triangulate(&self, src: &[f32], components: usize) -> Vec<f32> {
        let hole_mask = self.build_hole_mask();
        let total_tris: usize = self
            .face_vertex_counts
            .iter()
            .enumerate()
            .filter(|(fi, _)| !hole_mask.as_ref().is_some_and(|m| *fi < m.len() && m[*fi]))
            .map(|(_, &c)| (c as usize).saturating_sub(2))
            .sum();
        let mut dst = Vec::with_capacity(total_tris * 3 * components);
        let mut fvi_offset = 0usize;
        let n_verts = src.len() / components;

        for (face_idx, &count) in self.face_vertex_counts.iter().enumerate() {
            let n = count as usize;
            if hole_mask
                .as_ref()
                .is_some_and(|mask| face_idx < mask.len() && mask[face_idx])
            {
                fvi_offset += n;
                continue;
            }
            if n < 3 {
                fvi_offset += n;
                continue;
            }
            if fvi_offset + n > self.face_vertex_indices.len() {
                break;
            }
            // Fan triangulation using vertex indices to look up per-vertex data
            let hub = self.face_vertex_indices[fvi_offset] as usize;
            for i in 1..n - 1 {
                let (a, b) = if self.right_handed {
                    (
                        self.face_vertex_indices[fvi_offset + i] as usize,
                        self.face_vertex_indices[fvi_offset + i + 1] as usize,
                    )
                } else {
                    (
                        self.face_vertex_indices[fvi_offset + i + 1] as usize,
                        self.face_vertex_indices[fvi_offset + i] as usize,
                    )
                };
                // Copy vertex data, clamping OOB indices to zero
                let copy_vert = |vi: usize, dst: &mut Vec<f32>| {
                    if vi < n_verts {
                        dst.extend_from_slice(&src[vi * components..(vi + 1) * components]);
                    } else {
                        dst.extend(std::iter::repeat(0.0f32).take(components));
                    }
                };
                copy_vert(hub, &mut dst);
                copy_vert(a, &mut dst);
                copy_vert(b, &mut dst);
            }
            fvi_offset += n;
        }
        dst
    }

    fn build_hole_mask(&self) -> Option<Vec<bool>> {
        if self.hole_indices.is_empty() {
            return None;
        }
        let mut mask = vec![false; self.face_vertex_counts.len()];
        for &h in &self.hole_indices {
            if h < 0 {
                continue;
            }
            let idx = h as usize;
            if idx < mask.len() {
                mask[idx] = true;
            }
        }
        Some(mask)
    }

    /// Whether subdivision on this mesh produces triangles (Loop scheme).
    pub fn refines_to_triangles(&self) -> bool {
        self.subdivision_scheme == "loop"
    }
}

/// Vertex data for mesh rendering.
#[derive(Debug, Clone, Default)]
pub struct HdStMeshVertexData {
    /// Positions (3 floats per vertex)
    pub positions: Vec<f32>,
    /// Normals (3 floats per vertex, optional)
    pub normals: Vec<f32>,
    /// UVs (2 floats per vertex, optional)
    pub uvs: Vec<f32>,
    /// Per-vertex display color (3 floats per vertex, optional).
    /// From `displayColor` primvar — fallback color when no material is bound.
    pub colors: Vec<f32>,
    /// Previous-frame positions for motion blur (3 floats per vertex, optional).
    /// Populated when sample_primvar returns > 1 sample.
    /// Used to compute per-vertex motion vectors in the vertex shader.
    pub prev_positions: Vec<f32>,
}

impl HdStMeshVertexData {
    /// Get vertex count.
    pub fn get_vertex_count(&self) -> usize {
        self.positions.len() / 3
    }

    /// Get byte size of position data.
    pub fn get_positions_byte_size(&self) -> usize {
        self.positions.len() * std::mem::size_of::<f32>()
    }

    /// Get byte size of normals data.
    pub fn get_normals_byte_size(&self) -> usize {
        self.normals.len() * std::mem::size_of::<f32>()
    }

    /// Compute smooth normals from positions and triangle indices.
    ///
    /// Accumulates unnormalized face normals (cross products) into each vertex
    /// that references the triangle, then normalizes per vertex. This produces
    /// area-weighted average normals -- classic smooth shading, one normal per vertex.
    ///
    /// Used as a fallback when the USD mesh has no authored normals primvar.
    /// Matches the per-vertex layout required by the vertex-indexed draw path.
    ///
    /// Note: C++ Storm computes *flat* normals (one per face) on GPU via a compute
    /// shader (`HdSt_FlatNormalsComputationGPU`). Our CPU path produces smooth
    /// normals instead, which is a valid fallback for non-subdivision meshes.
    ///
    /// TODO(P2-R4): Edge-crease normal handling. Currently this function does a
    /// simple area-weighted average across ALL adjacent faces per vertex. For meshes
    /// with crease edges (SubdivTags::crease_indices / crease_sharpness), normals
    /// along crease edges should NOT be averaged across the crease boundary. The
    /// correct approach is:
    ///   1. Build an adjacency structure from crease_indices to identify crease edges.
    ///   2. When accumulating face normals, only average across faces that share
    ///      non-crease edges with the current face group.
    ///   3. Vertices on crease edges may need to be split (duplicated) so each
    ///      side of the crease gets its own normal.
    /// For subdivision meshes this is handled by the OpenSubdiv evaluator which
    /// produces limit normals respecting creases. This TODO only affects non-subdivision
    /// meshes that have authored crease data (rare but valid in USD).
    pub fn compute_smooth_normals(&mut self, triangles: &[u32]) {
        let vertex_count = self.get_vertex_count();
        self.normals = vec![0.0; vertex_count * 3];

        // Accumulate face normals per vertex
        for tri in triangles.chunks(3) {
            if tri.len() < 3 {
                continue;
            }

            let i0 = tri[0] as usize;
            let i1 = tri[1] as usize;
            let i2 = tri[2] as usize;

            // Bounds check: skip triangles with out-of-range vertex indices.
            if i0 * 3 + 2 >= self.positions.len()
                || i1 * 3 + 2 >= self.positions.len()
                || i2 * 3 + 2 >= self.positions.len()
            {
                continue;
            }

            // Get positions
            let p0 = [
                self.positions[i0 * 3],
                self.positions[i0 * 3 + 1],
                self.positions[i0 * 3 + 2],
            ];
            let p1 = [
                self.positions[i1 * 3],
                self.positions[i1 * 3 + 1],
                self.positions[i1 * 3 + 2],
            ];
            let p2 = [
                self.positions[i2 * 3],
                self.positions[i2 * 3 + 1],
                self.positions[i2 * 3 + 2],
            ];

            // Compute edge vectors
            let e1 = [p1[0] - p0[0], p1[1] - p0[1], p1[2] - p0[2]];
            let e2 = [p2[0] - p0[0], p2[1] - p0[1], p2[2] - p0[2]];

            // Cross product for face normal
            let n = [
                e1[1] * e2[2] - e1[2] * e2[1],
                e1[2] * e2[0] - e1[0] * e2[2],
                e1[0] * e2[1] - e1[1] * e2[0],
            ];

            // Add to vertex normals
            for &idx in &[i0, i1, i2] {
                self.normals[idx * 3] += n[0];
                self.normals[idx * 3 + 1] += n[1];
                self.normals[idx * 3 + 2] += n[2];
            }
        }

        // Normalize
        for i in 0..vertex_count {
            let nx = self.normals[i * 3];
            let ny = self.normals[i * 3 + 1];
            let nz = self.normals[i * 3 + 2];
            let len = (nx * nx + ny * ny + nz * nz).sqrt();
            if len > 1e-8 {
                self.normals[i * 3] /= len;
                self.normals[i * 3 + 1] /= len;
                self.normals[i * 3 + 2] /= len;
            }
        }
    }
}

/// Build a face-varying triangle map for the live mesh topology type used by
/// `HdStMesh`.
///
/// `usd-hd-st` still carries a second Storm topology type in `mesh_topology.rs`.
/// The runtime mesh path uses the local lightweight topology above, so the
/// face-varying fallback must derive its mapping from that authoritative state
/// instead of accidentally crossing topology domains.
fn build_fvar_triangle_index_map(topology: &HdStMeshTopology) -> Vec<u32> {
    let mut mapping = Vec::new();
    let mut offset = 0u32;

    for (face_idx, &count) in topology.face_vertex_counts.iter().enumerate() {
        let count = count as u32;

        if topology.hole_indices.contains(&(face_idx as i32)) {
            offset += count;
            continue;
        }

        if count < 3 {
            offset += count;
            continue;
        }

        for i in 1..count - 1 {
            mapping.push(offset);
            mapping.push(offset + i);
            mapping.push(offset + i + 1);
        }

        offset += count;
    }

    mapping
}

/// Retained authored face-varying primvars.
///
/// `_ref` keeps authored face-varying data separate from the core vertex payload and
/// uploads it through a dedicated `fvarBar`. The Rust port is still finishing the
/// shader/binder side, but it must already stop treating these authored arrays as
/// throwaway intermediate state inside the vertex BAR path.
#[derive(Debug, Clone, Default)]
struct FaceVaryingPrimvarData {
    normals: Vec<f32>,
    uvs: Vec<f32>,
    colors: Vec<f32>,
    opacity: Vec<f32>,
}

impl FaceVaryingPrimvarData {
    fn clear(&mut self) {
        self.normals.clear();
        self.uvs.clear();
        self.colors.clear();
        self.opacity.clear();
    }

    fn has_any(&self) -> bool {
        !self.normals.is_empty()
            || !self.uvs.is_empty()
            || !self.colors.is_empty()
            || !self.opacity.is_empty()
    }
}

/// Return true when authored primvar data matches the mesh's face-varying topology.
///
/// `_ref` keeps face-varying primvars distinct from vertex primvars as soon as they
/// are read from the scene delegate. Rust needs the same classification early so the
/// runtime does not accidentally feed face-varying buffers into vertex-only paths
/// like subdivision or smooth-normal generation.
fn is_face_varying_channel(count: usize, vertex_count: usize, face_varying_count: usize) -> bool {
    count > 0 && count != vertex_count && count == face_varying_count
}

/// Storm mesh representation.
///
/// Handles mesh rendering including:
/// - Vertex buffer management
/// - Index buffer management
/// - Subdivision surfaces (via opensubdiv-rs feature)
/// - Face-varying data (via opensubdiv-rs feature)
/// - GPU instancing (via mesh_sync instancer detection)
pub struct HdStMesh {
    /// Prim path
    path: SdfPath,

    /// Draw items for different representations
    draw_items: Vec<HdStDrawItemSharedPtr>,

    /// Mesh topology
    topology: HdStMeshTopology,

    /// Vertex data
    vertex_data: HdStMeshVertexData,

    /// Authoritative authored vertex primvars before any topology-driven cooking.
    ///
    /// `process_topology_cpu()` mutates positions and other vertex-domain channels
    /// for the current draw path. Keeping the authored state separate prevents
    /// topology-only resyncs from re-expanding already cooked data.
    authored_vertex_data: HdStMeshVertexData,

    /// Triangulated index buffer
    triangle_indices: Vec<u32>,

    /// Cached triangulated vertex indices before any face-varying fallback rewrite.
    ///
    /// The live fallback path still rewrites `triangle_indices` to `0..N` for
    /// sequential triangle-list draws when face-varying data is present. Retaining
    /// the original triangulation lets animated points recook the expanded vertex
    /// data without pretending topology changed every frame.
    triangulated_vertex_indices: Vec<u32>,

    /// Cached face-varying triangle map derived from the current topology.
    ///
    /// `_ref` keeps topology-derived computations registered off topology identity
    /// instead of rebuilding equivalent face-varying index walks every time a BAR
    /// upload happens. Rust does not have full registry-backed topology
    /// computations yet, but retaining this map on the mesh removes one repeated
    /// topology traversal from the live sync path.
    triangulated_fvar_indices: Vec<u32>,

    /// Vertex buffer resource (raw, used as fallback when no HGI)
    vertex_buffer: Option<HdStBufferResourceSharedPtr>,

    /// Index buffer resource (raw, used as fallback when no HGI)
    index_buffer: Option<HdStBufferResourceSharedPtr>,

    /// Managed vertex BAR from resource registry (preferred path)
    vertex_bar: Option<ManagedBarSharedPtr>,

    /// Managed element (index) BAR from resource registry (preferred path)
    element_bar: Option<ManagedBarSharedPtr>,

    /// Managed constant BAR for per-prim uniforms (transform, material ID)
    constant_bar: Option<ManagedBarSharedPtr>,

    /// Managed face-varying primvar BAR.
    ///
    /// This is the `_ref` `fvarBar` equivalent. It is currently populated only
    /// when a dedicated face-varying allocation exists, but the retained mesh
    /// object must still carry the slot so draw items can preserve parity.
    face_varying_bar: Option<ManagedBarSharedPtr>,

    /// Is visible
    visible: bool,

    /// Topology varies over time (animated topology — disable caching).
    /// Port of C++ _hasVaryingTopology (P2-8).
    has_varying_topology: bool,

    /// Per-vertex display opacity (from displayOpacity primvar, P2-7).
    /// Empty = fully opaque (default). One value per vertex.
    display_opacity: Vec<f32>,

    /// Authoritative authored display opacity before any topology-driven cooking.
    authored_display_opacity: Vec<f32>,

    /// Mapping from face-varying topology channels to authored primvar names.
    ///
    /// Port of `_ref` shared-data `fvarTopologyToPrimvarVector`. The Rust Storm
    /// path still needs a fuller topology tracker, but retaining this metadata on
    /// the mesh is required so draw items and binders can converge on the same
    /// contract as OpenUSD.
    fvar_topology_to_primvar_vector: TopologyToPrimvarVector,

    /// Retained authored face-varying primvars before any draw-path expansion.
    ///
    /// This is the mesh-side equivalent of `_ref` keeping face-varying primvars
    /// distinct from vertex primvars so they can feed `fvarBar` and related topology
    /// bindings instead of being lost during CPU flattening.
    face_varying_primvars: FaceVaryingPrimvarData,

    /// Topology is dirty
    topology_dirty: bool,

    /// Vertex data is dirty
    vertex_dirty: bool,

    /// Constant primvars need a registry-side upload on the next sync.
    ///
    /// This keeps `sync_from_delegate()` free of BAR mutations so the Storm
    /// batch path can remain a true read/process/upload split.
    constant_primvars_dirty: bool,

    /// World transform (local-to-world), row-major 4x4
    world_transform: [[f64; 4]; 4],

    /// Material params resolved from material binding.
    /// Matches C++ HdStMesh's use of _materialNetworkShader.
    material_params: MaterialParams,

    /// Subdivision refinement level from display style (0 = no subdivision).
    refine_level: u32,

    /// Cached material feature flags used by topology decisions.
    material_has_ptex: bool,
    material_has_limit_surface: bool,

    /// GPU texture+sampler handles for @group(3) texture binding.
    /// Populated by engine after texture load and HGI commit.
    texture_handles: MaterialTextureHandles,

    /// Current dirty bits for HdRprim trait bookkeeping.
    dirty_bits: HdDirtyBits,

    /// Instancer path, set when this mesh is instanced.
    instancer_id: Option<SdfPath>,

    /// Resource registry stored during initial sync so HdRprim::sync can reuse it.
    resource_registry: Option<Arc<HdStResourceRegistry>>,
}

impl std::fmt::Debug for HdStMesh {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HdStMesh")
            .field("path", &self.path)
            .field("visible", &self.visible)
            .field("topology_dirty", &self.topology_dirty)
            .field("vertex_dirty", &self.vertex_dirty)
            .field("draw_items", &self.draw_items.len())
            .field("has_resource_registry", &self.resource_registry.is_some())
            .finish()
    }
}

impl HdStMesh {
    /// Print and reset aggregate sync_from_delegate timing stats.
    pub fn flush_sync_stats() {
        SYNC_STATS.with(|s| {
            let mut s = s.borrow_mut();
            if s.count > 0 {
                log::info!("[PERF] mesh_sync: n={} delegate={:.0}ms topo={:.0}ms vert={:.0}ms | read: topo={:.0}ms xform={:.0}ms pts={:.0}ms nrm={:.0}ms other={:.0}ms desc={:.0}ms fvar={:.0}ms ext={:.0}ms",
                    s.count, s.delegate_ms, s.topo_ms, s.vert_ms,
                    s.topo_read_ms, s.xform_read_ms, s.points_read_ms, s.normals_read_ms, s.other_pv_ms,
                    s.primvar_desc_ms, s.fvar_meta_ms, s.ext_comp_ms);
                s.reset();
            }
        });
    }

    /// Create a new Storm mesh.
    pub fn new(path: SdfPath) -> Self {
        let id = [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ];
        Self {
            path,
            draw_items: Vec::new(),
            topology: HdStMeshTopology::new(),
            vertex_data: HdStMeshVertexData::default(),
            authored_vertex_data: HdStMeshVertexData::default(),
            triangle_indices: Vec::new(),
            triangulated_vertex_indices: Vec::new(),
            triangulated_fvar_indices: Vec::new(),
            vertex_buffer: None,
            index_buffer: None,
            vertex_bar: None,
            element_bar: None,
            constant_bar: None,
            face_varying_bar: None,
            visible: true,
            has_varying_topology: false,
            display_opacity: Vec::new(),
            authored_display_opacity: Vec::new(),
            fvar_topology_to_primvar_vector: TopologyToPrimvarVector::new(),
            face_varying_primvars: FaceVaryingPrimvarData::default(),
            topology_dirty: true,
            vertex_dirty: true,
            constant_primvars_dirty: true,
            refine_level: 0,
            material_params: MaterialParams::default(),
            material_has_ptex: false,
            material_has_limit_surface: false,
            texture_handles: MaterialTextureHandles::new(),
            world_transform: id,
            dirty_bits: usd_hd::change_tracker::HdRprimDirtyBits::ALL_DIRTY,
            instancer_id: None,
            resource_registry: None,
        }
    }

    /// Get prim path.
    pub fn get_path(&self) -> &SdfPath {
        &self.path
    }

    /// Set the local-to-world transform (row-major 4x4).
    pub fn set_world_transform(&mut self, xform: [[f64; 4]; 4]) {
        self.world_transform = xform;
    }

    /// Get the local-to-world transform (row-major 4x4).
    pub fn get_world_transform(&self) -> &[[f64; 4]; 4] {
        &self.world_transform
    }

    /// Returns the current local-space bounds of the synced vertex positions.
    ///
    /// This exposes the rprim's post-sync extent so higher layers can derive
    /// viewer bookkeeping from Hydra state instead of re-reading scene data.
    pub fn get_local_bbox(&self) -> ([f32; 3], [f32; 3]) {
        compute_aabb(&self.vertex_data.positions)
    }

    /// Set material params for this mesh (resolved from material binding).
    /// Matches C++ pattern: HdStMesh stores material ref, propagates to DrawItems.
    pub fn set_material_params(&mut self, params: MaterialParams) {
        self.material_params = params;
    }

    /// Store the resource registry for later use by HdRprim::sync during time-change re-sync.
    pub fn set_resource_registry(&mut self, registry: Arc<HdStResourceRegistry>) {
        self.resource_registry = Some(registry);
    }

    /// Set subdivision refinement level (0 = no subdivision).
    pub fn set_refine_level(&mut self, level: u32) {
        if self.refine_level != level {
            self.refine_level = level;
            self.topology_dirty = true;
        }
    }

    /// Get subdivision refinement level.
    pub fn get_refine_level(&self) -> u32 {
        self.refine_level
    }

    /// Set cached material features resolved by engine/material sprim.
    pub fn set_material_features(&mut self, has_ptex: bool, has_limit_surface: bool) {
        self.material_has_ptex = has_ptex;
        self.material_has_limit_surface = has_limit_surface;
    }

    /// Set GPU texture+sampler handles for @group(3) bind group.
    pub fn set_texture_handles(&mut self, handles: MaterialTextureHandles) {
        self.texture_handles = handles;
    }

    /// Get GPU texture+sampler handles.
    pub fn get_texture_handles(&self) -> &MaterialTextureHandles {
        &self.texture_handles
    }

    /// Get material params.
    pub fn get_material_params(&self) -> &MaterialParams {
        &self.material_params
    }

    /// Restore authored vertex-domain state before cooking topology-dependent draw data.
    ///
    /// The Rust fallback path still cooks topology directly on the retained mesh, so
    /// topology-only dirties must start from authored primvars instead of from the
    /// already-expanded draw buffers produced by the previous sync.
    fn restore_authored_vertex_state(&mut self) {
        self.vertex_data = self.authored_vertex_data.clone();
        self.display_opacity = self.authored_display_opacity.clone();
        self.topology
            .set_num_points(self.authored_vertex_data.get_vertex_count());
    }

    /// Return true when the simplified Rust draw path still needs the face-varying
    /// CPU fallback for this mesh.
    fn needs_face_varying_draw_fallback(&self) -> bool {
        self.face_varying_primvars.has_any()
    }

    /// Return true when the mesh is currently carrying the cooked sequential
    /// triangle-list topology used by the face-varying fallback path.
    fn is_using_face_varying_fallback_topology(&self) -> bool {
        !self.triangulated_vertex_indices.is_empty()
            && self.triangle_indices != self.triangulated_vertex_indices
    }

    /// Ensure the cached face-varying triangle map exists for the current topology.
    fn ensure_triangulated_fvar_indices(&mut self) -> bool {
        if self.triangulated_fvar_indices.is_empty() {
            self.triangulated_fvar_indices = build_fvar_triangle_index_map(&self.topology);
        }
        !self.triangulated_fvar_indices.is_empty()
    }

    /// Apply the simplified face-varying draw fallback to the current authored state.
    ///
    /// This keeps the live Rust path functional until `_ref`-style `fvarIndices` /
    /// `fvarPatchParam` plumbing lands. The helper is shared by topology sync and
    /// by animated-points recooks so both start from the same authored inputs.
    fn apply_face_varying_draw_fallback(&mut self, tri_vertex_indices: &[u32]) {
        if tri_vertex_indices.is_empty() || !self.ensure_triangulated_fvar_indices() {
            return;
        }

        let expansion_started = std::time::Instant::now();
        let tri_count = tri_vertex_indices.len() / 3;
        let vtx_count = self.vertex_data.get_vertex_count();
        let has_fv_normals = !self.face_varying_primvars.normals.is_empty();
        let has_fv_uvs = !self.face_varying_primvars.uvs.is_empty();
        let has_fv_colors = !self.face_varying_primvars.colors.is_empty();
        let has_fv_opacity = !self.face_varying_primvars.opacity.is_empty();

        let expanded_pos =
            expand_f32_components_by_indices(&self.vertex_data.positions, 3, tri_vertex_indices);
        self.vertex_data.positions = expanded_pos;
        self.topology.num_points = tri_count * 3;

        if has_fv_normals {
            self.vertex_data.normals.clear();
        } else if !self.vertex_data.normals.is_empty() {
            let expanded_nrm =
                expand_f32_components_by_indices(&self.vertex_data.normals, 3, tri_vertex_indices);
            self.vertex_data.normals = expanded_nrm;
        }

        if has_fv_uvs {
            self.vertex_data.uvs.clear();
        } else if !self.vertex_data.uvs.is_empty() {
            let expanded_uv =
                expand_f32_components_by_indices(&self.vertex_data.uvs, 2, tri_vertex_indices);
            self.vertex_data.uvs = expanded_uv;
        }

        let clr_count = self.vertex_data.colors.len() / 3;
        if has_fv_colors {
            self.vertex_data.colors.clear();
        } else if clr_count > 0 && clr_count != tri_count * 3 {
            let expanded_clr =
                expand_f32_components_by_indices(&self.vertex_data.colors, 3, tri_vertex_indices);
            self.vertex_data.colors = expanded_clr;
        }

        if has_fv_opacity {
            self.display_opacity.clear();
        } else if self.display_opacity.len() > 1 && self.display_opacity.len() == vtx_count {
            let expanded_opa =
                expand_f32_components_by_indices(&self.display_opacity, 1, tri_vertex_indices);
            self.display_opacity = expanded_opa;
        }

        self.triangle_indices = (0..tri_count as u32 * 3).collect();
        self.vertex_dirty = true;
        log::debug!(
            "HdStMesh::sync_topology: faceVarying fallback path={} vtx={} fvar_normals={} fvar_uvs={} fvar_colors={} fvar_opacity={} -> {} tri-verts in {:.2} ms",
            self.path,
            vtx_count,
            has_fv_normals,
            has_fv_uvs,
            has_fv_colors,
            has_fv_opacity,
            tri_count * 3,
            expansion_started.elapsed().as_secs_f64() * 1000.0
        );
    }

    /// Rebuild the cooked face-varying vertex data when points/vertex primvars changed
    /// but topology did not.
    fn recook_face_varying_draw_vertices(&mut self) {
        if !self.needs_face_varying_draw_fallback() {
            return;
        }
        self.restore_authored_vertex_state();
        if self.triangulated_vertex_indices.is_empty() {
            let (triangles, _) = self.topology.triangulate();
            self.triangulated_vertex_indices = triangles;
        }
        let tri_vertex_indices = self.triangulated_vertex_indices.clone();
        self.apply_face_varying_draw_fallback(&tri_vertex_indices);
    }

    /// Restore authored vertex buffers and the original triangulated topology when a
    /// mesh leaves the face-varying fallback path without a full topology dirty.
    fn restore_non_fvar_draw_topology(&mut self) {
        self.restore_authored_vertex_state();
        self.triangle_indices = self.triangulated_vertex_indices.clone();
        self.triangulated_fvar_indices.clear();
        self.topology_dirty = true;
        self.vertex_dirty = true;
    }

    /// Set mesh topology.
    pub fn set_topology(&mut self, topology: HdStMeshTopology) {
        self.topology = topology;
        self.topology_dirty = true;
    }

    /// Get mesh topology.
    pub fn get_topology(&self) -> &HdStMeshTopology {
        &self.topology
    }

    /// Set vertex positions.
    pub fn set_positions(&mut self, positions: Vec<f32>) {
        let num_pts = positions.len() / 3;
        self.vertex_data.positions = positions.clone();
        self.authored_vertex_data.positions = positions;
        // Keep topology vertex count in sync with the points array.
        self.topology.set_num_points(num_pts);
        self.vertex_dirty = true;
    }

    /// Set vertex normals.
    pub fn set_normals(&mut self, normals: Vec<f32>) {
        self.vertex_data.normals = normals.clone();
        self.authored_vertex_data.normals = normals;
        self.vertex_dirty = true;
    }

    /// Set UV (texcoord) primvar data (2 floats per vertex).
    pub fn set_uvs(&mut self, uvs: Vec<f32>) {
        self.vertex_data.uvs = uvs.clone();
        self.authored_vertex_data.uvs = uvs;
        self.vertex_dirty = true;
    }

    /// Get vertex count.
    pub fn get_vertex_count(&self) -> usize {
        self.vertex_data.get_vertex_count()
    }

    /// Get face count.
    pub fn get_face_count(&self) -> usize {
        self.topology.get_face_count()
    }

    /// Get triangle count (after triangulation).
    pub fn get_triangle_count(&self) -> usize {
        self.triangle_indices.len() / 3
    }

    /// Set visibility.
    pub fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
    }

    /// Check if visible.
    pub fn is_visible(&self) -> bool {
        self.visible
    }

    /// Mark topology as dirty.
    pub fn mark_topology_dirty(&mut self) {
        self.topology_dirty = true;
    }

    /// Mark vertex data as dirty.
    pub fn mark_vertex_dirty(&mut self) {
        self.vertex_dirty = true;
    }

    /// Check if topology is dirty.
    pub fn is_topology_dirty(&self) -> bool {
        self.topology_dirty
    }

    // --- P1-7: Material/topology query functions ---

    /// Returns true when the bound material uses Ptex textures.
    ///
    /// Port of C++ HdStMesh::_MaterialHasPtex. Used to choose face-varying
    /// UV topology vs per-vertex UV layout.
    pub fn material_has_ptex(&self) -> bool {
        self.material_has_ptex
    }

    /// Returns true when quad indices should be used instead of triangle indices.
    ///
    /// Port of C++ HdStMesh::_UseQuadIndices. Quads are preferred for Catmull-Clark
    /// subdivision as they preserve the coarse cage structure.
    pub fn use_quad_indices(&self) -> bool {
        // Never quadrangulate schemes that refine to triangles (Loop).
        if self.topology.refines_to_triangles() {
            return false;
        }
        self.material_has_ptex() || is_force_quadrangulate_enabled()
    }

    /// Returns true when the bound material has a displacement shader.
    ///
    /// Port of C++ HdStMesh::_MaterialHasLimitSurface. Limit surface evaluation
    /// requires OSD GPU path and provides smoother displacement.
    pub fn material_has_limit_surface(&self) -> bool {
        self.material_has_limit_surface
    }

    /// Returns true when limit surface refinement should be used.
    ///
    /// Port of C++ HdStMesh::_UseLimitRefinement. Returns true when subdivision
    /// scheme is Catmull-Clark and material has limit surface evaluation.
    pub fn use_limit_refinement(&self) -> bool {
        self.material_has_limit_surface()
    }

    /// Returns true when topology varies over time (animated, P2-8).
    pub fn has_varying_topology(&self) -> bool {
        self.has_varying_topology
    }

    /// Get per-vertex display opacity (from displayOpacity primvar, P2-7).
    pub fn get_display_opacity(&self) -> &[f32] {
        &self.display_opacity
    }

    /// Sync GPU resources after direct data set (set_topology/set_positions).
    ///
    /// Triangulates, computes normals, and uploads buffers to GPU.
    /// Use this when populating mesh data directly (not via scene delegate).
    ///
    /// `commit` controls whether pending uploads are flushed immediately.
    /// Pass `false` when syncing many meshes in a frame and committing once
    /// at a higher level for better batching.
    pub fn sync(&mut self, resource_registry: &HdStResourceRegistry, commit: bool) {
        let diag_sync = std::env::var_os("USD_PROFILE_MESH_SYNC").is_some();
        let path_text = self.path.to_string();
        let diag = |msg: &str| {
            if diag_sync {
                eprintln!("[HdStMesh::sync] path={} {msg}", path_text);
            }
        };
        if self.topology_dirty {
            diag("sync_topology");
            self.sync_topology(resource_registry);
            self.topology_dirty = false;
        } else if self.vertex_dirty && self.needs_face_varying_draw_fallback() {
            diag("recook_face_varying_draw_vertices");
            self.recook_face_varying_draw_vertices();
        } else if self.vertex_dirty && self.is_using_face_varying_fallback_topology() {
            diag("restore_non_fvar_draw_topology");
            self.restore_non_fvar_draw_topology();
            self.sync_topology(resource_registry);
            self.topology_dirty = false;
        }
        if self.vertex_dirty {
            diag("sync_vertices");
            self.sync_vertices(resource_registry);
            self.vertex_dirty = false;
        }
        if commit {
            diag("commit");
            resource_registry.commit();
        }
        diag("update_draw_items");
        self.update_draw_items();
    }

    /// Phase 1: Pure CPU topology + vertex processing.
    /// Triangulation, faceVarying expansion, smooth normals — no GPU/registry.
    /// Safe to call from multiple threads (operates on owned mesh data only).
    pub fn process_cpu(&mut self) {
        if self.topology_dirty {
            self.process_topology_cpu();
        } else if self.vertex_dirty && self.needs_face_varying_draw_fallback() {
            self.recook_face_varying_draw_vertices();
        } else if self.vertex_dirty && self.is_using_face_varying_fallback_topology() {
            self.restore_non_fvar_draw_topology();
            self.process_topology_cpu();
        }
        if self.vertex_dirty {
            self.process_vertices_cpu();
        }
    }

    /// Phase 2: Upload processed data to GPU via resource registry.
    /// Must be called sequentially (registry is shared).
    pub fn upload_to_registry(&mut self, resource_registry: &HdStResourceRegistry) {
        if self.topology_dirty {
            self.upload_topology(resource_registry);
            self.topology_dirty = false;
        }
        if self.vertex_dirty {
            self.upload_vertices(resource_registry);
            self.vertex_dirty = false;
        }
        if self.constant_primvars_dirty {
            self.sync_constant_primvars(resource_registry);
            self.constant_primvars_dirty = false;
        }
        self.update_draw_items();
    }

    /// Sync the mesh from the scene delegate by pulling authored state into the rprim.
    ///
    /// This stage reads topology, transform, points, normals, and authored primvars
    /// from the delegate and marks the mesh as needing CPU/GPU sync.
    ///
    /// Heavy topology processing and buffer uploads are intentionally performed by
    /// the caller's chosen execution path:
    /// - `HdStMesh::sync(...)` for direct/non-batched sync,
    /// - `process_cpu()` + `upload_to_registry()` for the batched Storm path.
    ///
    /// Mirrors the delegate-read portion of C++ `HdStMesh::Sync` /
    /// `_PopulateTopology` / `_PopulateVertexPrimvars`.
    pub fn sync_from_delegate(&mut self, delegate: &dyn HdSceneDelegate, dirty_bits: &mut HdDirtyBits) {
        usd_trace::trace_scope!("mesh_sync_from_delegate");
        let diag_sync = std::env::var_os("USD_PROFILE_MESH_SYNC").is_some();
        let path_text = self.path.to_string();
        let diag = |msg: &str| {
            if diag_sync {
                eprintln!("[HdStMesh::sync_from_delegate] path={} {msg}", path_text);
            }
        };
        diag("start");
        let original_dirty_bits = *dirty_bits;
        let _t_total = std::time::Instant::now();
        let mut _t_vis = std::time::Duration::ZERO;
        let mut _t_topo_read = std::time::Duration::ZERO;
        let mut _t_xform_read = std::time::Duration::ZERO;
        let mut _t_points_read = std::time::Duration::ZERO;
        let mut _t_normals_read = std::time::Duration::ZERO;
        let mut _t_other_primvars = std::time::Duration::ZERO;
        let mut _t_primvar_desc = std::time::Duration::ZERO;
        let mut _t_fvar_meta = std::time::Duration::ZERO;
        let mut _t_ext_comp = std::time::Duration::ZERO;
        let primvar_value_dirty = *dirty_bits & HdRprimDirtyBits::DIRTY_PRIMVAR != 0;
        let primvar_or_points_dirty =
            *dirty_bits & (HdRprimDirtyBits::DIRTY_PRIMVAR | HdRprimDirtyBits::DIRTY_POINTS) != 0;
        let topo_or_primvar_dirty =
            *dirty_bits & (HdRprimDirtyBits::DIRTY_TOPOLOGY | HdRprimDirtyBits::DIRTY_PRIMVAR) != 0;
        let _tdesc = std::time::Instant::now();
        diag("gather_primvar_descriptors");
        let primvar_descriptors = if primvar_value_dirty {
            gather_all_primvar_descriptors(delegate, &self.path)
        } else {
            Vec::new()
        };
        diag("gather_primvar_descriptors:done");
        let uv_primvar = find_texcoord_primvar_in_descriptors(&primvar_descriptors);
        let has_authored_normals = has_named_primvar_descriptor(&primvar_descriptors, "normals");
        let has_display_opacity =
            has_named_primvar_descriptor(&primvar_descriptors, "displayOpacity");
        let has_display_color = has_named_primvar_descriptor(&primvar_descriptors, "displayColor");
        _t_primvar_desc = _tdesc.elapsed();
        if *dirty_bits & (HdRprimDirtyBits::DIRTY_TOPOLOGY | HdRprimDirtyBits::DIRTY_PRIMVAR) != 0 {
            self.face_varying_primvars.clear();
        }
        // --- Visibility ---
        if *dirty_bits & HdRprimDirtyBits::DIRTY_VISIBILITY != 0 {
            diag("get_visible");
            let _tv = std::time::Instant::now();
            self.visible = delegate.get_visible(&self.path);
            _t_vis = _tv.elapsed();
        }

        // --- Topology (faceVertexCounts + faceVertexIndices) ---
        if *dirty_bits & HdRprimDirtyBits::DIRTY_TOPOLOGY != 0 {
            diag("get_mesh_topology");
            let _tt = std::time::Instant::now();
            let hd_topo = delegate.get_mesh_topology(&self.path);
            let right_handed = hd_topo.orientation != "leftHanded";
            // P2-8: track whether topology varies over time.
            // A topology with hole_indices is more likely to be animated (driven by
            // sculpt/simulation). This is a conservative heuristic — a full
            // implementation would compare the new topology against the cached one.
            let old_face_count = self.topology.face_vertex_counts.len();
            self.topology = HdStMeshTopology {
                face_vertex_counts: hd_topo.face_vertex_counts,
                face_vertex_indices: hd_topo.face_vertex_indices,
                hole_indices: hd_topo.hole_indices,
                right_handed,
                subdivision_scheme: hd_topo.scheme,
                subdiv_tags: hd_topo.subdiv_tags,
                // Keep authored point count stable across topology-only dirties so later
                // primvar cardinality classification does not see the cooked triangle-list
                // vertex count from a previous face-varying fallback expansion.
                num_points: self.authored_vertex_data.get_vertex_count(),
            };
            // Mark varying if face count changed (topology mutated since last sync).
            let new_face_count = self.topology.face_vertex_counts.len();
            if old_face_count > 0 && old_face_count != new_face_count {
                self.has_varying_topology = true;
            }
            self.topology_dirty = true;
            _t_topo_read = _tt.elapsed();
        }

        // --- Display style (refinement level) ---
        // Read refinement level from scene delegate display style.
        // This controls subdivision tessellation when scheme != "none".
        if *dirty_bits & (HdRprimDirtyBits::DIRTY_DISPLAY_STYLE | HdRprimDirtyBits::DIRTY_TOPOLOGY)
            != 0
        {
            let style = delegate.get_display_style(&self.path);
            let new_level = (style.refine_level.max(0) as u32).min(8);
            self.set_refine_level(new_level);
        }

        // --- Transform ---
        if *dirty_bits & HdRprimDirtyBits::DIRTY_TRANSFORM != 0 {
            diag("get_transform");
            let _tx = std::time::Instant::now();
            let xf = delegate.get_transform(&self.path);
            self.world_transform = xf.to_array();
            self.constant_primvars_dirty = true;
            _t_xform_read = _tx.elapsed();
        }

        // --- Primvars: points (positions) ---
        //
        // `_ref` Storm reads current-time authored primvars on the main mesh sync
        // path and handles motion-blur resolution elsewhere. Keeping multi-sample
        // point fetches here was both slower and a semantic divergence.
        if primvar_or_points_dirty {
            diag("get_points");
            let _tp = std::time::Instant::now();
            let points_key = usd_tf::Token::new("points");
            let points_value = delegate.get(&self.path, &points_key);
            if let Some(pts) = points_value.as_vec_clone::<usd_gf::Vec3f>() {
                let positions: Vec<f32> = pts.iter().flat_map(|v| [v.x, v.y, v.z]).collect();
                self.vertex_data.positions = positions.clone();
                self.authored_vertex_data.positions = positions;
                // Store explicit vertex count so topology bounds-checks are O(1).
                self.topology.set_num_points(pts.len());
                self.vertex_dirty = true;
            }
            // Keep the old field empty on the regular sync path. Motion-blur data is
            // expected to come from the dedicated velocity/motion resolving stage.
            self.vertex_data.prev_positions.clear();
            self.authored_vertex_data.prev_positions.clear();

            // Trace: delegate data readback
            let _pos_count = self.vertex_data.positions.len() / 3;
            let _fvc_len = self.topology.face_vertex_counts.len();
            let _fvi_len = self.topology.face_vertex_indices.len();
            let _fvi_max = self
                .topology
                .face_vertex_indices
                .iter()
                .copied()
                .max()
                .unwrap_or(0);
            log::trace!(
                "HdStMesh::sync_from_delegate: fvc={} fvi={} positions={} fvi_max={}",
                _fvc_len,
                _fvi_len,
                _pos_count,
                _fvi_max
            );
            if _pos_count > 0 && _fvi_max as usize >= _pos_count {
                log::warn!(
                    "HdStMesh::sync_from_delegate: INDEX OOB! max(fvi)={} >= vertex_count={}",
                    _fvi_max,
                    _pos_count
                );
            }

            _t_points_read = _tp.elapsed();
            let _tn = std::time::Instant::now();
            if primvar_value_dirty {
                if has_authored_normals {
                    diag("get_normals");
                    let normals_key = usd_tf::Token::new("normals");
                    let (val, opt_idx) = delegate.get_indexed_primvar(&self.path, &normals_key);
                    if let Some(nrm) = val.as_vec_clone::<usd_gf::Vec3f>() {
                        let expanded = expand_indexed_vec3f(&nrm, opt_idx.as_deref());
                        let flat: Vec<f32> =
                            expanded.iter().flat_map(|v| [v.x, v.y, v.z]).collect();
                        let normal_count = flat.len() / 3;
                        let vertex_count = self.topology.get_vertex_count();
                        let face_varying_count = self.topology.face_vertex_indices.len();
                        if is_face_varying_channel(normal_count, vertex_count, face_varying_count) {
                            self.face_varying_primvars.normals = flat;
                            self.vertex_data.normals.clear();
                            self.authored_vertex_data.normals.clear();
                        } else {
                            self.vertex_data.normals = flat.clone();
                            self.authored_vertex_data.normals = flat;
                        }
                    }
                } else {
                    self.face_varying_primvars.normals.clear();
                    self.vertex_data.normals.clear();
                    self.authored_vertex_data.normals.clear();
                }
            }
            _t_normals_read = _tn.elapsed();

            let _to = std::time::Instant::now();
            if primvar_value_dirty {
                if has_display_opacity {
                    diag("get_display_opacity");
                    let opacity_key = usd_tf::Token::new("displayOpacity");
                    let (val, opt_idx) = delegate.get_indexed_primvar(&self.path, &opacity_key);
                    if let Some(opacities) = val.as_vec_clone::<f32>() {
                        let expanded = expand_indexed_f32(&opacities, opt_idx.as_deref());
                        if !expanded.is_empty() {
                            let vertex_count = self.topology.get_vertex_count();
                            let face_varying_count = self.topology.face_vertex_indices.len();
                            if is_face_varying_channel(
                                expanded.len(),
                                vertex_count,
                                face_varying_count,
                            ) {
                                self.face_varying_primvars.opacity = expanded;
                                self.display_opacity.clear();
                                self.authored_display_opacity.clear();
                            } else {
                                self.display_opacity = expanded.clone();
                                self.authored_display_opacity = expanded;
                            }
                        }
                    }
                } else {
                    self.face_varying_primvars.opacity.clear();
                    self.display_opacity.clear();
                    self.authored_display_opacity.clear();
                }

                if has_display_color {
                    diag("get_display_color");
                    let color_key = usd_tf::Token::new("displayColor");
                    let (val, opt_idx) = delegate.get_indexed_primvar(&self.path, &color_key);
                    if let Some(colors) = val.as_vec_clone::<usd_gf::Vec3f>() {
                        let expanded = expand_indexed_vec3f(&colors, opt_idx.as_deref());
                        if !expanded.is_empty() {
                            let flat: Vec<f32> =
                                expanded.iter().flat_map(|v| [v.x, v.y, v.z]).collect();
                            let color_count = flat.len() / 3;
                            let vertex_count = self.topology.get_vertex_count();
                            let face_varying_count = self.topology.face_vertex_indices.len();
                            if is_face_varying_channel(
                                color_count,
                                vertex_count,
                                face_varying_count,
                            ) {
                                self.face_varying_primvars.colors = flat;
                                self.vertex_data.colors.clear();
                                self.authored_vertex_data.colors.clear();
                            } else {
                                self.vertex_data.colors = flat.clone();
                                self.authored_vertex_data.colors = flat;
                            }
                        }
                    }
                } else {
                    self.face_varying_primvars.colors.clear();
                    self.vertex_data.colors.clear();
                    self.authored_vertex_data.colors.clear();
                }

                if let Some(ref key) = uv_primvar {
                    diag("get_uvs");
                    let (val, opt_indices) = delegate.get_indexed_primvar(&self.path, key);
                    if let Some(uvs) = val.as_vec_clone::<usd_gf::Vec2f>() {
                        if !uvs.is_empty() {
                            let expanded = if let Some(ref indices) = opt_indices {
                                indices
                                    .iter()
                                    .map(|&i| {
                                        let idx = i as usize;
                                        if idx < uvs.len() {
                                            uvs[idx]
                                        } else {
                                            usd_gf::Vec2f::default()
                                        }
                                    })
                                    .collect::<Vec<_>>()
                            } else {
                                uvs
                            };
                            let flat: Vec<f32> =
                                expanded.iter().flat_map(|v| [v.x, v.y]).collect();
                            let uv_count = flat.len() / 2;
                            let vertex_count = self.topology.get_vertex_count();
                            let face_varying_count = self.topology.face_vertex_indices.len();
                            log::debug!(
                                "HdStMesh::sync: UV '{}' expanded={} verts",
                                key.as_str(),
                                uv_count
                            );
                            if is_face_varying_channel(
                                uv_count,
                                vertex_count,
                                face_varying_count,
                            ) {
                                self.face_varying_primvars.uvs = flat;
                                self.vertex_data.uvs.clear();
                                self.authored_vertex_data.uvs.clear();
                            } else {
                                self.vertex_data.uvs = flat.clone();
                                self.authored_vertex_data.uvs = flat;
                            }
                        }
                    }
                } else {
                    self.face_varying_primvars.uvs.clear();
                    self.vertex_data.uvs.clear();
                    self.authored_vertex_data.uvs.clear();
                }

                // Any DIRTY_PRIMVAR read can change vertex BAR contents or the retained
                // face-varying BAR contract, even when points themselves stayed clean.
                self.vertex_dirty = true;
            }
            _t_other_primvars = _to.elapsed();
        }

        if topo_or_primvar_dirty {
            diag("gather_fvar_topology_metadata");
            let _tfv = std::time::Instant::now();
            self.fvar_topology_to_primvar_vector =
                gather_face_varying_topology_metadata_from_data(
                    &self.face_varying_primvars,
                    &self.topology,
                );
            _t_fvar_meta = _tfv.elapsed();
        }

        // --- ExtComputation primvars (GPU / CPU) ---
        // Query the scene delegate for ext-computation primvar descriptors.
        // This still mirrors `_ref` delegate discovery, but the actual registry
        // scheduling remains a follow-up gap; keep phase 1 read-only here.
        if *dirty_bits & HdRprimDirtyBits::DIRTY_PRIMVAR != 0 {
            diag("get_ext_computation_primvars");
            let _te = std::time::Instant::now();
            let mut cpu_sources: Vec<BufferSourceSharedPtr> = Vec::new();
            let mut gpu_reserve = Vec::new();
            let mut gpu_computations = Vec::new();

            if let Some(resource_registry) = self.resource_registry.as_deref() {
                get_ext_computation_primvars_computations(
                    &self.path,
                    delegate,
                    *dirty_bits,
                    resource_registry,
                    &mut cpu_sources,
                    &mut gpu_reserve,
                    &mut gpu_computations,
                );
            } else {
                log::trace!(
                    "HdStMesh::sync ext_comp skipped path={} reason=no_resource_registry",
                    self.path,
                );
            }

            if !cpu_sources.is_empty() || !gpu_reserve.is_empty() || !gpu_computations.is_empty() {
                log::trace!(
                    "HdStMesh::sync ext_comp pending path={} cpu_sources={} gpu_reserve={} gpu_computations={}",
                    self.path,
                    cpu_sources.len(),
                    gpu_reserve.len(),
                    gpu_computations.len(),
                );
            }
            _t_ext_comp = _te.elapsed();
        }

        let _t_delegate = _t_total.elapsed();
        let _t_topo_ms = 0.0f64;
        let _t_vert_ms = 0.0f64;

        let _t_all = _t_total.elapsed();
        SYNC_STATS.with(|s| {
            let mut s = s.borrow_mut();
            s.count += 1;
            s.delegate_ms += _t_delegate.as_secs_f64() * 1000.0;
            s.topo_ms += _t_topo_ms;
            s.vert_ms += _t_vert_ms;
            s.commit_ms += 0.0; // commit deferred to engine level
            s.vis_ms += _t_vis.as_secs_f64() * 1000.0;
            s.topo_read_ms += _t_topo_read.as_secs_f64() * 1000.0;
            s.xform_read_ms += _t_xform_read.as_secs_f64() * 1000.0;
            s.points_read_ms += _t_points_read.as_secs_f64() * 1000.0;
            s.normals_read_ms += _t_normals_read.as_secs_f64() * 1000.0;
            s.other_pv_ms += _t_other_primvars.as_secs_f64() * 1000.0;
            s.primvar_desc_ms += _t_primvar_desc.as_secs_f64() * 1000.0;
            s.fvar_meta_ms += _t_fvar_meta.as_secs_f64() * 1000.0;
            s.ext_comp_ms += _t_ext_comp.as_secs_f64() * 1000.0;
        });

        let vertex_count = self.vertex_data.get_vertex_count();
        let normal_count = self.vertex_data.normals.len() / 3;
        let uv_count = self.vertex_data.uvs.len() / 2;
        let fvi_count = self.topology.face_vertex_indices.len();
        let has_fv_normals = !self.face_varying_primvars.normals.is_empty();
        let has_fv_uvs = !self.face_varying_primvars.uvs.is_empty();
        let total_ms = _t_all.as_secs_f64() * 1000.0;
        if total_ms >= 10.0 || has_fv_normals || has_fv_uvs {
            log::debug!(
                "HdStMesh::sync_from_delegate summary path={} dirty={:?} total_ms={:.2} delegate_ms={:.2} topo_ms={:.2} vert_ms={:.2} primvar_desc_ms={:.2} fvar_meta_ms={:.2} ext_comp_ms={:.2} verts={} normals={} uvs={} fvi={} has_fv_normals={} has_fv_uvs={}",
                self.path,
                original_dirty_bits,
                total_ms,
                _t_delegate.as_secs_f64() * 1000.0,
                _t_topo_ms,
                _t_vert_ms,
                _t_primvar_desc.as_secs_f64() * 1000.0,
                _t_fvar_meta.as_secs_f64() * 1000.0,
                _t_ext_comp.as_secs_f64() * 1000.0,
                vertex_count,
                normal_count,
                uv_count,
                fvi_count,
                has_fv_normals,
                has_fv_uvs,
            );
        }

        // Clear processed dirty bits (mirrors C++ *dirtyBits &= ~AllSceneDirtyBits)
        *dirty_bits &= !HdRprimDirtyBits::ALL_SCENE_DIRTY_BITS;
        diag("done");
    }
    /// Sync topology using the BAR system.
    ///
    /// Allocates/updates a ManagedBar for the index data, queues a BufferSource
    /// for upload via resource_registry.commit(). Falls back to raw buffer alloc
    /// when no resource registry BAR system is needed (headless/mock).
    fn sync_topology(&mut self, resource_registry: &HdStResourceRegistry) {
        self.process_topology_cpu();
        self.upload_topology(resource_registry);
    }

    /// Pure CPU: subdivide, triangulate, faceVarying expand.
    /// Thread-safe — only touches owned mesh data.
    fn process_topology_cpu(&mut self) {
        let _t0 = std::time::Instant::now();
        self.restore_authored_vertex_state();
        // Apply subdivision if scheme is not "none" and refine_level > 0.
        // This refines positions + normals and replaces topology with subdivided result.
        #[cfg(feature = "subdivision")]
        {
            let scheme = self.topology.subdivision_scheme.as_str();
            if self.refine_level > 0 && scheme != "none" {
                if let Some(result) = crate::subdivision::subdivide_mesh(
                    scheme,
                    &self.topology.face_vertex_counts,
                    &self.topology.face_vertex_indices,
                    &self.topology.hole_indices,
                    &self.topology.subdiv_tags,
                    &self.vertex_data.positions,
                    &self.vertex_data.normals,
                    self.refine_level,
                ) {
                    log::debug!(
                        "HdStMesh::sync_topology: subdivided scheme={} level={} verts {} -> {} faces {} -> {}",
                        scheme,
                        self.refine_level,
                        self.vertex_data.positions.len() / 3,
                        result.num_vertices,
                        self.topology.face_vertex_counts.len(),
                        result.face_vertex_counts.len(),
                    );
                    // Replace topology and vertex data with refined result
                    self.topology.face_vertex_counts = result.face_vertex_counts;
                    self.topology.face_vertex_indices = result.face_vertex_indices;
                    self.vertex_data.positions = result.positions;
                    // Update vertex count to match refined topology
                    self.topology.num_points = result.num_vertices as usize;
                    if !result.normals.is_empty() {
                        self.vertex_data.normals = result.normals;
                    }
                    // Force normals recompute since vertex count changed
                    self.vertex_dirty = true;
                }
            }
        }

        let _t_subdiv = _t0.elapsed();
        // Triangulate mesh (original or subdivided)
        let _t1 = std::time::Instant::now();
        let (triangles, _tri_count) = self.topology.triangulate();
        self.triangulated_vertex_indices = triangles.clone();
        self.triangle_indices = triangles;
        let _t_triangulate = _t1.elapsed();

        if self.triangle_indices.is_empty() {
            self.triangulated_vertex_indices.clear();
            self.triangulated_fvar_indices.clear();
            return;
        }

        // The live WGSL path now consumes authored face-varying channels from `fvarBar`,
        // but the simplified Rust renderer still expands positions and rewrites triangle
        // indices for triangle-list draws. Keep that fallback isolated and only pay the
        // extra topology work on meshes that actually carry face-varying primvars.
        if self.needs_face_varying_draw_fallback() {
            let tri_vertex_indices = self.triangulated_vertex_indices.clone();
            self.apply_face_varying_draw_fallback(&tri_vertex_indices);
        } else {
            self.triangulated_fvar_indices.clear();
        }
    }

    /// Upload processed topology (triangle indices) to GPU via resource registry.
    fn upload_topology(&mut self, resource_registry: &HdStResourceRegistry) {
        let _t0 = std::time::Instant::now();
        let index_count = self.triangle_indices.len();
        let element_size = std::mem::size_of::<u32>(); // 4 bytes per index
        let index_size = index_count * element_size;

        // Convert triangle_indices to raw bytes for buffer source
        let index_bytes = encode_u32_slice_le(&self.triangle_indices);

        let specs = vec![BufferSpec {
            name: Token::new("indices"),
            num_elements: index_count,
            element_size,
        }];
        let usage = BufferArrayUsageHint {
            index: true,
            ..Default::default()
        };

        // Allocate or update element BAR
        let element_bar = resource_registry.update_non_uniform_bar(
            &Token::new("topology"),
            self.element_bar.as_ref(),
            &specs,
            &[],
            usage,
        );

        // Queue source for GPU upload at next commit()
        let source = std::sync::Arc::new(BufferSource::new(
            Token::new("indices"),
            index_bytes,
            index_count,
            element_size,
        ));
        resource_registry.add_source(&element_bar, source);

        // Keep a raw buffer reference for the draw_item BAR wrapper
        let raw_buf = {
            let locked = element_bar.lock().expect("element_bar lock");
            locked.buffer.clone()
        };
        self.index_buffer = Some(raw_buf);
        self.element_bar = Some(element_bar);

        log::debug!(
            "HdStMesh::upload_topology: {} triangles, {} bytes (BAR)",
            index_count / 3,
            index_size
        );
        #[cfg(debug_assertions)]
        {
            let max_tri_idx = self.triangle_indices.iter().copied().max().unwrap_or(0);
            let vtx_count = self.topology.get_vertex_count();
            if max_tri_idx as usize >= vtx_count {
                log::warn!(
                    "HdStMesh::sync_topology: INDEX OOB! max(tri_idx)={} >= vertex_count={}",
                    max_tri_idx,
                    vtx_count
                );
            }
        }
    }

    /// Sync vertex data using the BAR system.
    ///
    /// Allocates/updates ManagedBars for positions and normals, queues
    /// BufferSources for GPU upload at next commit().
    fn sync_vertices(&mut self, resource_registry: &HdStResourceRegistry) {
        self.process_vertices_cpu();
        self.upload_vertices(resource_registry);
    }

    /// Pure CPU: compute smooth normals if needed.
    /// Thread-safe — only touches owned mesh data.
    fn process_vertices_cpu(&mut self) {
        if self.vertex_data.normals.is_empty()
            && self.face_varying_primvars.normals.is_empty()
            && !self.triangle_indices.is_empty()
        {
            self.vertex_data
                .compute_smooth_normals(&self.triangle_indices);
        }
    }

    /// Upload vertex data (positions, normals, UVs, colors) to GPU via resource registry.
    fn upload_vertices(&mut self, resource_registry: &HdStResourceRegistry) {
        let vertex_count = self.vertex_data.get_vertex_count();
        if vertex_count == 0 {
            return;
        }

        let float_size = std::mem::size_of::<f32>(); // 4 bytes
        let vec3_size = 3 * float_size; // 12 bytes per vec3
        let vec2_size = 2 * float_size; // 8 bytes per vec2

        // Build combined specs: positions + normals + uvs + colors packed into one BAR.
        // Use actual data sizes — UVs may be face-varying (more elements than positions).
        let has_normals = !self.vertex_data.normals.is_empty();
        let has_uvs = !self.vertex_data.uvs.is_empty();
        let has_colors = !self.vertex_data.colors.is_empty();
        let nrm_count = self.vertex_data.normals.len() / 3;
        let uv_count = self.vertex_data.uvs.len() / 2;
        let color_count = self.vertex_data.colors.len() / 3;
        let mut specs = vec![BufferSpec {
            name: Token::new("points"),
            num_elements: vertex_count,
            element_size: vec3_size,
        }];
        if has_normals {
            specs.push(BufferSpec {
                name: Token::new("normals"),
                num_elements: nrm_count,
                element_size: vec3_size,
            });
        }
        if has_uvs {
            specs.push(BufferSpec {
                name: Token::new("st"),
                num_elements: uv_count,
                element_size: vec2_size,
            });
        }
        if has_colors {
            specs.push(BufferSpec {
                name: Token::new("displayColor"),
                num_elements: color_count,
                element_size: vec3_size,
            });
        }

        let usage = BufferArrayUsageHint {
            vertex: true,
            ..Default::default()
        };

        // Allocate or update vertex BAR
        let vertex_bar = resource_registry.update_non_uniform_bar(
            &Token::new("vertex"),
            self.vertex_bar.as_ref(),
            &specs,
            &[],
            usage,
        );

        // Queue positions source
        let pos_bytes = encode_f32_slice_le(&self.vertex_data.positions);
        let pos_source = std::sync::Arc::new(BufferSource::new(
            Token::new("points"),
            pos_bytes,
            vertex_count,
            vec3_size,
        ));
        // Queue all vertex sources in one batch so commit() writes them
        // at consecutive offsets within the shared BAR buffer.
        let mut vertex_sources: Vec<BufferSourceSharedPtr> = vec![pos_source];
        if has_normals {
            let nrm_bytes = encode_f32_slice_le(&self.vertex_data.normals);
            let nrm_source = std::sync::Arc::new(BufferSource::new(
                Token::new("normals"),
                nrm_bytes,
                nrm_count,
                vec3_size,
            ));
            vertex_sources.push(nrm_source);
        }
        if has_uvs {
            let uv_bytes = encode_f32_slice_le(&self.vertex_data.uvs);
            let uv_source = std::sync::Arc::new(BufferSource::new(
                Token::new("st"),
                uv_bytes,
                uv_count,
                vec2_size,
            ));
            vertex_sources.push(uv_source);
        }
        if has_colors {
            let color_bytes = encode_f32_slice_le(&self.vertex_data.colors);
            let color_source = std::sync::Arc::new(BufferSource::new(
                Token::new("displayColor"),
                color_bytes,
                color_count,
                vec3_size,
            ));
            vertex_sources.push(color_source);
        }
        resource_registry.add_sources(&vertex_bar, vertex_sources);

        // Keep raw buffer ref for draw_item BAR wrapper
        let raw_buf = {
            let locked = vertex_bar.lock().expect("vertex_bar lock");
            locked.buffer.clone()
        };
        self.vertex_buffer = Some(raw_buf);
        self.vertex_bar = Some(vertex_bar);
        self.upload_face_varying_primvars(resource_registry);

        log::debug!(
            "HdStMesh::sync_vertices: {} vertices, has_normals={} has_uvs={} has_colors={} (BAR)",
            vertex_count,
            has_normals,
            has_uvs,
            has_colors
        );

        // Trace: vertex sync diagnostics
        let pos_bytes_len = self.vertex_data.positions.len() * std::mem::size_of::<f32>();
        let nrm_bytes_len = self.vertex_data.normals.len() * std::mem::size_of::<f32>();
        log::trace!(
            "HdStMesh::sync_vertices: vtx={} has_nrm={} pos_bytes={} nrm_bytes={} specs=[points: n={} esz={}, normals: n={} esz={}] sources_queued={}",
            vertex_count,
            has_normals,
            pos_bytes_len,
            nrm_bytes_len,
            vertex_count,
            vec3_size,
            if has_normals { vertex_count } else { 0 },
            if has_normals { vec3_size } else { 0 },
            if has_normals { 2 } else { 1 }
        );
    }

    /// Upload retained authored face-varying primvars into a dedicated BAR.
    ///
    /// This is the Rust-side equivalent of `_ref` `fvarBar` population. The draw path
    /// still has follow-up work in binder/WGSL, but the data must already live in its
    /// own buffer allocation instead of existing only as a transient CPU rewrite.
    fn upload_face_varying_primvars(&mut self, resource_registry: &HdStResourceRegistry) {
        const FVAR_HEADER_WORDS: usize = 4;
        const FVAR_SLOT_NORMALS: usize = 0;
        const FVAR_SLOT_UV: usize = 1;
        const FVAR_SLOT_COLOR: usize = 2;
        const FVAR_SLOT_OPACITY: usize = 3;

        if !self.face_varying_primvars.has_any() {
            self.face_varying_bar = None;
            return;
        }

        if self.triangulated_fvar_indices.is_empty() {
            self.triangulated_fvar_indices = build_fvar_triangle_index_map(&self.topology);
        }
        if self.triangulated_fvar_indices.is_empty() {
            self.face_varying_bar = None;
            return;
        }

        let mut header_words = [u32::MAX; FVAR_HEADER_WORDS];
        let mut payload_words: Vec<u32> = Vec::new();

        let mut append_channel = |slot: usize, expanded: Vec<f32>| {
            if expanded.is_empty() {
                return;
            }
            header_words[slot] = (FVAR_HEADER_WORDS + payload_words.len()) as u32;
            payload_words.extend(expanded.into_iter().map(f32::to_bits));
        };

        if !self.face_varying_primvars.normals.is_empty() {
            append_channel(
                FVAR_SLOT_NORMALS,
                expand_f32_components_by_indices(
                    &self.face_varying_primvars.normals,
                    3,
                    &self.triangulated_fvar_indices,
                ),
            );
        }

        if !self.face_varying_primvars.uvs.is_empty() {
            append_channel(
                FVAR_SLOT_UV,
                expand_f32_components_by_indices(
                    &self.face_varying_primvars.uvs,
                    2,
                    &self.triangulated_fvar_indices,
                ),
            );
        }

        if !self.face_varying_primvars.colors.is_empty() {
            append_channel(
                FVAR_SLOT_COLOR,
                expand_f32_components_by_indices(
                    &self.face_varying_primvars.colors,
                    3,
                    &self.triangulated_fvar_indices,
                ),
            );
        }

        if !self.face_varying_primvars.opacity.is_empty() {
            append_channel(
                FVAR_SLOT_OPACITY,
                expand_f32_components_by_indices(
                    &self.face_varying_primvars.opacity,
                    1,
                    &self.triangulated_fvar_indices,
                ),
            );
        }

        if payload_words.is_empty() {
            self.face_varying_bar = None;
            return;
        }

        let mut packed_words = header_words.to_vec();
        packed_words.extend(payload_words);
        let packed_bytes = encode_u32_slice_le(&packed_words);

        let specs = vec![BufferSpec {
            name: Token::new("faceVaryingData"),
            num_elements: packed_words.len(),
            element_size: std::mem::size_of::<u32>(),
        }];
        let sources: Vec<BufferSourceSharedPtr> = vec![Arc::new(BufferSource::new(
            Token::new("faceVaryingData"),
            packed_bytes,
            packed_words.len(),
            std::mem::size_of::<u32>(),
        ))];

        let usage = BufferArrayUsageHint {
            storage: true,
            ..Default::default()
        };
        let face_varying_bar = resource_registry.update_non_uniform_bar(
            &Token::new("faceVarying"),
            self.face_varying_bar.as_ref(),
            &specs,
            &[],
            usage,
        );
        resource_registry.add_sources(&face_varying_bar, sources);
        self.face_varying_bar = Some(face_varying_bar);
    }

    /// Sync constant primvars: per-prim uniforms (world transform, material ID).
    ///
    /// Port of HdStMesh::_PopulateConstantPrimvars. Allocates a constant BAR
    /// and queues the world transform matrix for GPU upload.
    fn sync_constant_primvars(&mut self, resource_registry: &HdStResourceRegistry) {
        // Pack world transform (4x4 f64 → 4x4 f32 = 16 floats = 64 bytes)
        let mat_f32: Vec<f32> = self
            .world_transform
            .iter()
            .flat_map(|row| row.iter().map(|&v| v as f32))
            .collect();
        let mat_bytes = encode_f32_slice_le(&mat_f32);

        let element_size = std::mem::size_of::<f32>(); // 4 bytes
        let specs = vec![BufferSpec {
            name: Token::new("transform"),
            num_elements: 16, // 4x4 matrix = 16 floats
            element_size,
        }];
        let usage = BufferArrayUsageHint {
            uniform: true,
            ..Default::default()
        };

        // Allocate or update constant BAR
        let constant_bar = resource_registry.update_uniform_bar(
            &Token::new("constant"),
            self.constant_bar.as_ref(),
            &specs,
            &[],
            usage,
        );

        // Queue transform upload
        let source = std::sync::Arc::new(BufferSource::new(
            Token::new("transform"),
            mat_bytes,
            16,
            element_size,
        ));
        resource_registry.add_source(&constant_bar, source);
        self.constant_bar = Some(constant_bar);
    }

    /// Update draw items with current buffer references.
    ///
    /// Builds HdStBufferArrayRange wrappers from managed BARs or raw buffers,
    /// sets vertex/element/constant BARs on the draw item.
    ///
    /// Unlike the old implementation, this keeps existing draw-item objects
    /// alive and updates them in place. This matches OpenUSD's ownership model
    /// more closely: reprs own long-lived draw items, and sync updates their
    /// bindings rather than replacing the objects every frame.
    fn update_draw_items(&mut self) {
        let diag_sync = std::env::var_os("USD_PROFILE_MESH_SYNC").is_some();
        let path_text = self.path.to_string();
        let diag = |msg: &str| {
            if diag_sync {
                eprintln!("[HdStMesh::update_draw_items] path={} {msg}", path_text);
            }
        };
        diag("start");
        // Determine vertex buffer + size
        let (vbuf, vbuf_size, vbuf_offset) = if let Some(ref vbar) = self.vertex_bar {
            diag("lock_vertex_bar");
            let locked = vbar.lock().expect("vertex_bar lock");
            if !locked.is_valid() {
                return;
            }
            (locked.buffer.clone(), locked.byte_size(), locked.offset)
        } else if let Some(ref raw) = self.vertex_buffer {
            if !raw.is_valid() {
                return;
            }
            let sz = raw.get_size();
            (raw.clone(), sz, 0)
        } else {
            return;
        };

        // Determine element (index) buffer + size
        let (ibuf, ibuf_size, ibuf_offset) = if let Some(ref ebar) = self.element_bar {
            diag("lock_element_bar");
            let locked = ebar.lock().expect("element_bar lock");
            if !locked.is_valid() {
                return;
            }
            (locked.buffer.clone(), locked.byte_size(), locked.offset)
        } else if let Some(ref raw) = self.index_buffer {
            if !raw.is_valid() {
                return;
            }
            let sz = raw.get_size();
            (raw.clone(), sz, 0)
        } else {
            return;
        };

        if vbuf_size == 0 || ibuf_size == 0 {
            return;
        }

        // Wrap in HdStBufferArrayRange — pass per-stream byte sizes so
        // draw_batch can bind pos/normals/uvs/colors at the correct offsets.
        let pos_byte_size = self.vertex_data.get_positions_byte_size();
        let nrm_byte_size = self.vertex_data.get_normals_byte_size();
        let uv_byte_size = self.vertex_data.uvs.len() * std::mem::size_of::<f32>();
        let color_byte_size = self.vertex_data.colors.len() * std::mem::size_of::<f32>();
        let vertex_bar: HdBufferArrayRangeSharedPtr =
            Arc::new(HdStBufferArrayRange::with_stream_sizes(
                vbuf,
                vbuf_offset,
                vbuf_size,
                pos_byte_size,
                nrm_byte_size,
                uv_byte_size,
                color_byte_size,
            ));
        let element_bar: HdBufferArrayRangeSharedPtr =
            Arc::new(HdStBufferArrayRange::new(ibuf, ibuf_offset, ibuf_size));
        let constant_bar: Option<HdBufferArrayRangeSharedPtr> =
            self.constant_bar.as_ref().and_then(|cbar| {
                diag("lock_constant_bar");
                let locked = cbar.lock().expect("constant_bar lock");
                if !locked.is_valid() {
                    return None;
                }
                Some(Arc::new(HdStBufferArrayRange::new(
                    locked.buffer.clone(),
                    locked.offset,
                    locked.byte_size(),
                )) as HdBufferArrayRangeSharedPtr)
            });
        let face_varying_bar: Option<HdBufferArrayRangeSharedPtr> =
            self.face_varying_bar.as_ref().and_then(|fbar| {
                diag("lock_face_varying_bar");
                let locked = fbar.lock().expect("face_varying_bar lock");
                if !locked.is_valid() {
                    return None;
                }
                Some(Arc::new(HdStBufferArrayRange::new(
                    locked.buffer.clone(),
                    locked.offset,
                    locked.byte_size(),
                )) as HdBufferArrayRangeSharedPtr)
            });

        // Trace: draw item buffer layout diagnostics
        let vertex_count = self.vertex_data.get_vertex_count();
        let max_tri_idx = self.triangle_indices.iter().copied().max().unwrap_or(0);
        log::trace!(
            "HdStMesh::update_draw_items: vbuf_size={} vbuf_off={} ibuf_size={} ibuf_off={} pos_bytes={} vtx_count={} max_tri_idx={}",
            vbuf_size,
            vbuf_offset,
            ibuf_size,
            ibuf_offset,
            pos_byte_size,
            vertex_count,
            max_tri_idx
        );
        if vertex_count > 0 && max_tri_idx as usize >= vertex_count {
            log::warn!(
                "HdStMesh::update_draw_items: INDEX OOB! max(tri_idx)={} >= vertex_count={} — expect stretched triangles!",
                max_tri_idx,
                vertex_count
            );
        }
        let bbox = compute_aabb(&self.vertex_data.positions);

        if self.draw_items.is_empty() {
            diag("allocate_default_draw_item");
            let item = HdStDrawItem::new(self.path.clone());
            item.set_repr(Token::new("refined"));
            self.draw_items.push(Arc::new(item));
        }

        diag("populate_draw_items");
        for item in &self.draw_items {
            item.set_vertex_bar(vertex_bar.clone());
            item.set_element_bar(element_bar.clone());
            item.clear_constant_bar();
            if let Some(ref constant_bar) = constant_bar {
                item.set_constant_bar(constant_bar.clone());
            }
            item.clear_face_varying_bar();
            if let Some(ref face_varying_bar) = face_varying_bar {
                item.set_face_varying_bar(face_varying_bar.clone());
            }
            item.set_fvar_topology_to_primvar_vector(self.fvar_topology_to_primvar_vector.clone());
            item.set_visible(self.visible);
            item.set_material_network_shader(self.material_params.clone());
            item.set_texture_handles(self.texture_handles.clone());
            item.set_bbox(bbox.0, bbox.1);
            item.set_world_transform(self.world_transform);
        }
        diag("done");
    }

    /// Return the initial set of dirty bits that triggers a full sync.
    ///
    /// Port of C++ HdStMesh::GetInitialDirtyBitsMask.
    /// Called by the render index when a mesh prim is first inserted to set
    /// up the change tracker with all the state that needs to be populated.
    pub fn get_initial_dirty_bits_mask() -> HdDirtyBits {
        use usd_hd::change_tracker::HdRprimDirtyBits;
        HdRprimDirtyBits::ALL_SCENE_DIRTY_BITS
            | dirty_bits::DIRTY_SMOOTH_NORMALS
            | dirty_bits::DIRTY_FLAT_NORMALS
            | dirty_bits::DIRTY_INDICES
            | dirty_bits::DIRTY_HULL_INDICES
    }

    /// Initialize representation state for a given repr token.
    ///
    /// Port of C++ HdStMesh::_InitRepr. Called when a new representation
    /// (e.g. "smoothHull", "refinedWireOnSurf") is first requested.
    /// Creates draw items per repr descriptor and marks appropriate dirty bits.
    ///
    /// C++ creates draw items based on _MeshReprConfig::DescArray per repr.
    /// We create repr-specific draw items with appropriate topology settings.
    pub fn init_repr(&mut self, repr_token: &Token, dirty_bits: &mut HdDirtyBits) {
        use usd_hd::change_tracker::HdRprimDirtyBits;

        // Skip if repr already initialised
        if self
            .draw_items
            .iter()
            .any(|item| item.get_repr() == *repr_token)
        {
            return;
        }

        // Mark as needing a new repr sync pass (port of C++ NewRepr)
        *dirty_bits |= HdRprimDirtyBits::DIRTY_REPR;

        // Get repr descriptors and create draw items per descriptor.
        // Port of C++ _GetReprDesc(reprToken) + draw item allocation loop.
        let descs = get_mesh_repr_descs(repr_token);

        for desc in &descs {
            if desc.geom_style == MeshGeomStyle::Invalid {
                continue;
            }

            // Create draw item for this repr descriptor
            let draw_item = HdStDrawItem::new(self.path.clone());
            draw_item.set_repr(repr_token.clone());

            // Set dirty bits based on geom style (port of C++ topology index switch)
            match desc.geom_style {
                MeshGeomStyle::Hull
                | MeshGeomStyle::HullEdgeOnly
                | MeshGeomStyle::HullEdgeOnSurf => {
                    *dirty_bits |= dirty_bits::DIRTY_HULL_INDICES;
                }
                MeshGeomStyle::Points => {
                    *dirty_bits |= dirty_bits::DIRTY_POINTS_INDICES;
                }
                _ => {
                    *dirty_bits |= dirty_bits::DIRTY_INDICES;
                }
            }

            // Flat vs smooth normals per descriptor
            if desc.flat_shading {
                *dirty_bits |= dirty_bits::DIRTY_FLAT_NORMALS;
            } else {
                *dirty_bits |= dirty_bits::DIRTY_SMOOTH_NORMALS;
            }

            self.draw_items.push(Arc::new(draw_item));
        }

        log::trace!(
            "HdStMesh::init_repr: repr={} path={} draw_items={}",
            repr_token.as_str(),
            self.path,
            self.draw_items.len(),
        );
    }

    /// Update the representation when dirty bits indicate a change.
    ///
    /// Port of C++ HdStMesh::_UpdateRepr. Called by `Sync` whenever the
    /// mesh or its representation is dirty. Drives topology, primvar, and
    /// normal recomputation for the requested repr.
    pub fn update_repr(
        &mut self,
        repr_token: &Token,
        dirty_bits: &mut HdDirtyBits,
        resource_registry: &HdStResourceRegistry,
    ) {
        // Sync topology if needed
        let needs_hull = matches!(
            repr_token.as_str(),
            "hull" | "wireOnSurf" | "refinedWireOnSurf"
        );
        if *dirty_bits & dirty_bits::DIRTY_HULL_INDICES != 0 || (needs_hull && self.topology_dirty)
        {
            self.sync_topology(resource_registry);
            self.topology_dirty = false;
            *dirty_bits &= !dirty_bits::DIRTY_HULL_INDICES;
        } else if *dirty_bits & dirty_bits::DIRTY_INDICES != 0 || self.topology_dirty {
            self.sync_topology(resource_registry);
            self.topology_dirty = false;
            *dirty_bits &= !dirty_bits::DIRTY_INDICES;
        }

        // Sync vertex primvars
        if *dirty_bits & dirty_bits::DIRTY_SMOOTH_NORMALS != 0 || self.vertex_dirty {
            self.sync_vertices(resource_registry);
            self.vertex_dirty = false;
            *dirty_bits &= !(dirty_bits::DIRTY_SMOOTH_NORMALS | dirty_bits::DIRTY_FLAT_NORMALS);
        }

        // NOTE: commit deferred to HdEngine::Execute commit_resources phase.
        self.update_draw_items();
    }

    /// Get vertex buffer resource.
    pub fn get_vertex_buffer(&self) -> Option<&HdStBufferResourceSharedPtr> {
        self.vertex_buffer.as_ref()
    }

    /// Get index buffer resource.
    pub fn get_index_buffer(&self) -> Option<&HdStBufferResourceSharedPtr> {
        self.index_buffer.as_ref()
    }

    /// Get triangle indices.
    pub fn get_triangle_indices(&self) -> &[u32] {
        &self.triangle_indices
    }

    /// Get draw items for a representation.
    ///
    /// Returns draw items matching the given representation token.
    /// Standard repr values include:
    /// - "refined" - subdivision surface (default)
    /// - "hull" - control cage
    /// - "wireOnSurf" - wireframe overlay
    /// - "points" - point cloud view
    pub fn get_draw_items(&self, repr: &Token) -> Vec<HdStDrawItemSharedPtr> {
        self.draw_items
            .iter()
            .filter(|item| item.get_repr() == *repr)
            .cloned()
            .collect()
    }

    /// Add a draw item.
    pub fn add_draw_item(&mut self, item: HdStDrawItemSharedPtr) {
        self.draw_items.push(item);
    }

    /// Get all draw items.
    pub fn get_all_draw_items(&self) -> &[HdStDrawItemSharedPtr] {
        &self.draw_items
    }

    /// Refresh repr-owned draw items from the mesh's current CPU-side state.
    ///
    /// Hydra sync updates the rprim first; engine-side bookkeeping that adjusts
    /// material or texture bindings afterwards can call this to keep the
    /// long-lived draw items coherent without replacing them.
    pub fn refresh_draw_item_bindings(&mut self) {
        self.update_draw_items();
    }

    /// Create N instance draw items sharing the same vertex/index BARs.
    ///
    /// Each draw item gets a synthetic path `<mesh_path>.__inst_<i>` and has
    /// `instance_bar` set (a dummy marker) so that `PipelineDrawBatch` detects
    /// `use_instancing=true` and issues a single GPU-instanced draw call.
    ///
    /// Call AFTER `sync()` / `update_draw_items()` so that BARs are available.
    pub fn create_instance_draw_items(&mut self, count: usize) {
        if count <= 1 || self.draw_items.is_empty() {
            return;
        }

        // Get BARs from the first (prototype) draw item.
        let proto_item = &self.draw_items[0];
        let vertex_bar = proto_item.get_vertex_bar();
        let element_bar = proto_item.get_element_bar();
        let constant_bar = proto_item.get_constant_bar();
        let mat_shader = proto_item.get_material_network_shader();
        let tex_handles = proto_item.get_texture_handles();
        let bbox_min = proto_item.get_bbox_min();
        let bbox_max = proto_item.get_bbox_max();

        // Mark the prototype draw item as instanced too.
        if let Some(ref vbar) = vertex_bar {
            // Use vertex_bar as instance_bar marker (same buffer, just a signal)
            self.draw_items[0].set_instance_bar(vbar.clone());
        }

        // Create N-1 additional draw items (prototype is already item 0).
        for i in 1..count {
            let inst_path = self
                .path
                .append_child(&format!("__inst_{}", i))
                .unwrap_or_else(|| self.path.clone());
            let item = HdStDrawItem::new(inst_path);
            if let Some(ref bar) = vertex_bar {
                item.set_vertex_bar(bar.clone());
                item.set_instance_bar(bar.clone()); // marker for use_instancing
            }
            if let Some(ref bar) = element_bar {
                item.set_element_bar(bar.clone());
            }
            if let Some(ref bar) = constant_bar {
                item.set_constant_bar(bar.clone());
            }
            item.set_material_network_shader(mat_shader.clone());
            item.set_texture_handles(tex_handles.clone());

            item.set_bbox(bbox_min, bbox_max);
            item.set_visible(true);
            self.draw_items.push(Arc::new(item));
        }

        log::debug!(
            "[HdStMesh] created {} instance draw items for {}",
            count,
            self.path
        );
    }

    /// Upload vertex and index data to GPU via resource registry.
    ///
    /// Uses HGI CopyBufferCpuToGpu when registry has Hgi; otherwise no-op for mock handles.
    /// Caller must invoke `resource_registry.submit_blit_work()` after all mesh uploads.
    pub fn upload_to_gpu(&self, resource_registry: &HdStResourceRegistry) {
        if resource_registry.get_hgi().is_none() {
            return; // Mock handles, nothing to upload
        }

        // Upload index buffer via HGI blit
        if let Some(ref buffer) = self.index_buffer {
            if !self.triangle_indices.is_empty() {
                let byte_size = self.triangle_indices.len() * std::mem::size_of::<u32>();
                // SAFETY: triangle_indices lives until submit_blit_work
                #[allow(unsafe_code)]
                unsafe {
                    resource_registry.copy_buffer_cpu_to_gpu(
                        buffer.get_handle(),
                        self.triangle_indices.as_ptr() as *const u8,
                        byte_size,
                        0,
                    );
                }
            }
        }

        // Upload vertex buffer (positions + normals) via HGI blit
        if let Some(ref buffer) = self.vertex_buffer {
            let position_size = self.vertex_data.get_positions_byte_size();
            let normal_size = self.vertex_data.get_normals_byte_size();

            if !self.vertex_data.positions.is_empty() {
                // SAFETY: positions data lives until submit_blit_work
                #[allow(unsafe_code)]
                unsafe {
                    resource_registry.copy_buffer_cpu_to_gpu(
                        buffer.get_handle(),
                        self.vertex_data.positions.as_ptr() as *const u8,
                        position_size,
                        0,
                    );
                }
            }
            if !self.vertex_data.normals.is_empty() {
                // SAFETY: normals data lives until submit_blit_work
                #[allow(unsafe_code)]
                unsafe {
                    resource_registry.copy_buffer_cpu_to_gpu(
                        buffer.get_handle(),
                        self.vertex_data.normals.as_ptr() as *const u8,
                        normal_size,
                        position_size,
                    );
                }
            }
            if !self.vertex_data.uvs.is_empty() {
                let uv_size = self.vertex_data.uvs.len() * std::mem::size_of::<f32>();
                // UVs follow positions + normals in the packed buffer
                let uv_offset = position_size + normal_size;
                // SAFETY: uvs data lives until submit_blit_work
                #[allow(unsafe_code)]
                unsafe {
                    resource_registry.copy_buffer_cpu_to_gpu(
                        buffer.get_handle(),
                        self.vertex_data.uvs.as_ptr() as *const u8,
                        uv_size,
                        uv_offset,
                    );
                }
            }
        }
    }
}

/// Compute an AABB from flat f32 positions (x,y,z triples).
///
/// Returns (min, max) in local space. When `positions` is empty, returns
/// the C++ convention for an empty bbox: ([FLT_MAX..], [-FLT_MAX..]).
/// The GPU culling shader treats min > max as "unbounded — always visible".
fn compute_aabb(positions: &[f32]) -> ([f32; 3], [f32; 3]) {
    if positions.len() < 3 {
        return ([f32::MAX; 3], [f32::MIN; 3]);
    }
    let mut mn = [f32::MAX, f32::MAX, f32::MAX];
    let mut mx = [f32::MIN, f32::MIN, f32::MIN];
    for chunk in positions.chunks_exact(3) {
        if chunk[0] < mn[0] {
            mn[0] = chunk[0];
        }
        if chunk[1] < mn[1] {
            mn[1] = chunk[1];
        }
        if chunk[2] < mn[2] {
            mn[2] = chunk[2];
        }
        if chunk[0] > mx[0] {
            mx[0] = chunk[0];
        }
        if chunk[1] > mx[1] {
            mx[1] = chunk[1];
        }
        if chunk[2] > mx[2] {
            mx[2] = chunk[2];
        }
    }
    (mn, mx)
}

fn is_force_quadrangulate_enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| {
        std::env::var("HD_ENABLE_FORCE_QUADRANGULATE")
            .ok()
            .is_some_and(|v| v == "1")
    })
}

fn encode_u32_slice_le(data: &[u32]) -> Vec<u8> {
    // x86_64 is always little-endian — zero-copy reinterpret
    #[cfg(target_endian = "little")]
    {
        let byte_len = data.len() * std::mem::size_of::<u32>();
        let ptr = data.as_ptr() as *const u8;
        // SAFETY: u32 is POD, alignment is weaker (u8), length is exact.
        let slice = unsafe { std::slice::from_raw_parts(ptr, byte_len) };
        slice.to_vec()
    }
    #[cfg(not(target_endian = "little"))]
    {
        let mut out = Vec::with_capacity(data.len() * std::mem::size_of::<u32>());
        for &v in data {
            out.extend_from_slice(&v.to_le_bytes());
        }
        out
    }
}

fn encode_f32_slice_le(data: &[f32]) -> Vec<u8> {
    #[cfg(target_endian = "little")]
    {
        let byte_len = data.len() * std::mem::size_of::<f32>();
        let ptr = data.as_ptr() as *const u8;
        let slice = unsafe { std::slice::from_raw_parts(ptr, byte_len) };
        slice.to_vec()
    }
    #[cfg(not(target_endian = "little"))]
    {
        let mut out = Vec::with_capacity(data.len() * std::mem::size_of::<f32>());
        for &v in data {
            out.extend_from_slice(&v.to_le_bytes());
        }
        out
    }
}

/// Shared pointer to Storm mesh.
pub type HdStMeshSharedPtr = Arc<HdStMesh>;

impl usd_hd::prim::rprim::HdRprim for HdStMesh {
    fn get_id(&self) -> &SdfPath {
        &self.path
    }

    fn get_dirty_bits(&self) -> HdDirtyBits {
        self.dirty_bits
    }

    fn set_dirty_bits(&mut self, bits: HdDirtyBits) {
        self.dirty_bits = bits;
    }

    fn get_instancer_id(&self) -> Option<&SdfPath> {
        self.instancer_id.as_ref()
    }

    fn init_repr(&mut self, repr_token: &Token, dirty_bits: &mut HdDirtyBits) {
        // Delegate to the existing Storm-specific init_repr implementation.
        self.init_repr(repr_token, dirty_bits);
    }

    fn sync(
        &mut self,
        delegate: &dyn usd_hd::prim::HdSceneDelegate,
        _render_param: Option<&dyn usd_hd::render::render_delegate::HdRenderParam>,
        dirty_bits: &mut HdDirtyBits,
        repr_token: &Token,
    ) {
        // Use the registry captured during the initial populate phase.
        // If it's not set yet (edge case), skip rather than panic.
        if let Some(registry) = self.resource_registry.clone() {
            self.sync_from_delegate(delegate, dirty_bits);
            // Call the concrete GPU-upload method (not this trait method).
            HdStMesh::sync(self, &registry, false);
        } else {
            log::warn!("[HdStMesh] HdRprim::sync called before resource_registry was set on {}", self.path);
        }
        let _ = repr_token; // repr_token drives init_repr, not the sync itself
    }

    fn is_visible(&self) -> bool {
        self.visible
    }
}

/// Expand indexed primvar `Vec3f` through optional indices.
/// If no indices, returns the input unchanged.
fn expand_indexed_vec3f(values: &[usd_gf::Vec3f], indices: Option<&[i32]>) -> Vec<usd_gf::Vec3f> {
    match indices {
        Some(idx) => idx.iter().map(|&i| {
            let ix = i as usize;
            if ix < values.len() { values[ix] } else { usd_gf::Vec3f::default() }
        }).collect(),
        None => values.to_vec(),
    }
}

/// Expand indexed primvar `f32` through optional indices.
fn expand_indexed_f32(values: &[f32], indices: Option<&[i32]>) -> Vec<f32> {
    match indices {
        Some(idx) => idx.iter().map(|&i| {
            let ix = i as usize;
            if ix < values.len() { values[ix] } else { 0.0 }
        }).collect(),
        None => values.to_vec(),
    }
}

/// Expand tightly-packed tuple data by a triangle or face-varying index map.
///
/// `components` is the tuple width in `f32` scalars. This is used by the current
/// CPU fallback path to reuse a single precomputed triangulation map across points,
/// normals, UVs, colors, and opacity instead of repeatedly traversing topology.
fn expand_f32_components_by_indices(src: &[f32], components: usize, indices: &[u32]) -> Vec<f32> {
    let mut result = Vec::with_capacity(indices.len() * components);
    for &index in indices {
        let base = index as usize * components;
        if base + components <= src.len() {
            result.extend_from_slice(&src[base..base + components]);
        } else {
            result.extend(std::iter::repeat(0.0).take(components));
        }
    }
    result
}

/// Collect primvar descriptors across every interpolation bucket.
///
/// `_ref` Storm resolves optional channels from descriptor lists, not by
/// probing likely names. Keeping that contract here avoids brute-force
/// per-primvar sampling on every mesh sync.
fn gather_all_primvar_descriptors(
    delegate: &dyn HdSceneDelegate,
    path: &SdfPath,
) -> Vec<HdPrimvarDescriptor> {
    const ALL_INTERPOLATIONS: [HdInterpolation; 6] = [
        HdInterpolation::Constant,
        HdInterpolation::Uniform,
        HdInterpolation::Varying,
        HdInterpolation::Vertex,
        HdInterpolation::FaceVarying,
        HdInterpolation::Instance,
    ];

    let mut descriptors = Vec::new();
    for interpolation in ALL_INTERPOLATIONS {
        descriptors.extend(delegate.get_primvar_descriptors(path, interpolation));
    }
    descriptors
}

/// Return true when the descriptor list contains the named primvar.
fn has_named_primvar_descriptor(descriptors: &[HdPrimvarDescriptor], name: &str) -> bool {
    descriptors.iter().any(|descriptor| descriptor.name == name)
}

/// Resolve the authored texture-coordinate primvar from already collected
/// descriptor sets.
fn find_texcoord_primvar_in_descriptors(
    descriptors: &[HdPrimvarDescriptor],
) -> Option<usd_tf::Token> {
    descriptors
        .iter()
        .find(|descriptor| descriptor.role == *usd_hd::schema::ROLE_TEXTURE_COORDINATE)
        .map(|descriptor| descriptor.name.clone())
}

/// Rebuild retained face-varying topology metadata from the currently authored
/// face-varying channels on the mesh itself.
fn gather_face_varying_topology_metadata_from_data(
    data: &FaceVaryingPrimvarData,
    topology: &HdStMeshTopology,
) -> TopologyToPrimvarVector {
    if !data.has_any() || topology.face_vertex_indices.is_empty() {
        return TopologyToPrimvarVector::new();
    }

    let mut primvars = Vec::new();
    if !data.normals.is_empty() {
        primvars.push(Token::new("normals"));
    }
    if !data.uvs.is_empty() {
        primvars.push(Token::new("st"));
    }
    if !data.colors.is_empty() {
        primvars.push(Token::new("displayColor"));
    }
    if !data.opacity.is_empty() {
        primvars.push(Token::new("displayOpacity"));
    }
    if primvars.is_empty() {
        return TopologyToPrimvarVector::new();
    }
    vec![TopologyToPrimvarEntry {
        topology: topology.face_vertex_indices.clone(),
        primvars,
    }]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::draw_item::HdStDrawItem;
    use parking_lot::RwLock;
    use usd_tf::Token;

    #[test]
    fn test_mesh_creation() {
        let path = SdfPath::from_string("/mesh").unwrap();
        let mesh = HdStMesh::new(path.clone());

        assert_eq!(mesh.get_path(), &path);
        assert_eq!(mesh.get_vertex_count(), 0);
        assert_eq!(mesh.get_face_count(), 0);
        assert!(mesh.is_visible());
        assert!(mesh.is_topology_dirty());
    }

    #[test]
    fn test_topology_triangulation() {
        // A simple quad face
        let topo = HdStMeshTopology::from_faces(vec![4], vec![0, 1, 2, 3]);

        let (triangles, count) = topo.triangulate();
        assert_eq!(count, 2);
        assert_eq!(triangles.len(), 6);

        // Check first triangle: 0, 1, 2
        assert_eq!(triangles[0], 0);
        assert_eq!(triangles[1], 1);
        assert_eq!(triangles[2], 2);

        // Check second triangle: 0, 2, 3
        assert_eq!(triangles[3], 0);
        assert_eq!(triangles[4], 2);
        assert_eq!(triangles[5], 3);
    }

    #[test]
    fn test_topology_with_triangle() {
        let topo = HdStMeshTopology::from_faces(vec![3], vec![0, 1, 2]);

        let (triangles, count) = topo.triangulate();
        assert_eq!(count, 1);
        assert_eq!(triangles, vec![0, 1, 2]);
    }

    #[test]
    fn test_topology_mixed_faces() {
        // Quad + triangle
        let topo = HdStMeshTopology::from_faces(vec![4, 3], vec![0, 1, 2, 3, 4, 5, 6]);

        let (triangles, count) = topo.triangulate();
        assert_eq!(count, 3); // 2 from quad + 1 from triangle
        assert_eq!(triangles.len(), 9);
    }

    #[test]
    fn test_mesh_sync() {
        let path = SdfPath::from_string("/mesh").unwrap();
        let mut mesh = HdStMesh::new(path);
        let registry = HdStResourceRegistry::new();

        // Set up a simple quad
        mesh.set_topology(HdStMeshTopology::from_faces(vec![4], vec![0, 1, 2, 3]));
        mesh.set_positions(vec![
            0.0, 0.0, 0.0, // v0
            1.0, 0.0, 0.0, // v1
            1.0, 1.0, 0.0, // v2
            0.0, 1.0, 0.0, // v3
        ]);

        mesh.sync(&registry, true);

        assert!(!mesh.is_topology_dirty());
        assert_eq!(mesh.get_triangle_count(), 2);
        assert!(mesh.get_vertex_buffer().is_some());
        assert!(mesh.get_index_buffer().is_some());
    }

    #[test]
    fn test_smooth_normal_computation() {
        let mut vertex_data = HdStMeshVertexData {
            positions: vec![
                0.0, 0.0, 0.0, // v0
                1.0, 0.0, 0.0, // v1
                0.0, 1.0, 0.0, // v2
            ],
            normals: Vec::new(),
            uvs: Vec::new(),
            colors: Vec::new(),
            prev_positions: Vec::new(),
        };

        let triangles = vec![0u32, 1, 2];
        vertex_data.compute_smooth_normals(&triangles);

        // Normal should point in +Z direction
        assert_eq!(vertex_data.normals.len(), 9);
        // All vertices share the same face normal
        for i in 0..3 {
            assert!(vertex_data.normals[i * 3 + 2].abs() > 0.9); // Z component ~ 1.0
        }
    }

    #[test]
    fn test_visibility() {
        let path = SdfPath::from_string("/mesh").unwrap();
        let mut mesh = HdStMesh::new(path);

        assert!(mesh.is_visible());
        mesh.set_visible(false);
        assert!(!mesh.is_visible());
    }

    #[test]
    fn test_use_quad_indices_material_ptex() {
        let path = SdfPath::from_string("/mesh").unwrap();
        let mut mesh = HdStMesh::new(path);
        let mut topo = HdStMeshTopology::from_faces(vec![4], vec![0, 1, 2, 3]);
        topo.subdivision_scheme = Token::new("catmullClark");
        mesh.set_topology(topo);
        mesh.set_material_features(true, false);
        assert!(mesh.use_quad_indices());
    }

    #[test]
    fn test_use_quad_indices_loop_scheme_disabled() {
        let path = SdfPath::from_string("/mesh").unwrap();
        let mut mesh = HdStMesh::new(path);
        let mut topo = HdStMeshTopology::from_faces(vec![3], vec![0, 1, 2]);
        topo.subdivision_scheme = Token::new("loop");
        mesh.set_topology(topo);
        mesh.set_material_features(true, false);
        assert!(!mesh.use_quad_indices());
    }

    #[test]
    fn test_use_limit_refinement_material_flag() {
        let path = SdfPath::from_string("/mesh").unwrap();
        let mut mesh = HdStMesh::new(path);
        mesh.set_material_features(false, true);
        assert!(mesh.use_limit_refinement());
    }

    #[test]
    fn test_draw_items() {
        let path = SdfPath::from_string("/mesh").unwrap();
        let mut mesh = HdStMesh::new(path.clone());

        let item = Arc::new(HdStDrawItem::new(path));
        mesh.add_draw_item(item);

        assert_eq!(mesh.get_all_draw_items().len(), 1);
    }

    // ---------------------------------------------------------------
    // Integration: sync → commit → draw_items pipeline invariants
    // ---------------------------------------------------------------
    // These tests verify the full sync→commit→update_draw_items chain
    // that broke when commit() shrunk BAR sizes.

    #[test]
    fn test_sync_commit_bar_size_preserved() {
        // Full pipeline: mesh sync allocates BAR (pos+nrm), commit processes
        // sources, then update_draw_items reads BAR size for draw items.
        // BAR size must equal positions + normals throughout.
        let path = SdfPath::from_string("/test_mesh").unwrap();
        let mut mesh = HdStMesh::new(path);
        let registry = HdStResourceRegistry::new();

        // 4 verts quad -> 2 triangles -> 6 vertices (flat normals)
        mesh.set_topology(HdStMeshTopology::from_faces(vec![4], vec![0, 1, 2, 3]));
        mesh.set_positions(vec![
            0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 1.0, 0.0, 0.0, 1.0, 0.0,
        ]);
        mesh.sync(&registry, true);

        // After sync, vertex BAR must exist
        let vbar = mesh.vertex_bar.as_ref().expect("vertex_bar after sync");
        let bar_before = {
            let locked = vbar.lock().unwrap();
            (locked.num_elements, locked.byte_size())
        };

        // Commit must NOT change BAR size
        registry.commit();

        let bar_after = {
            let locked = vbar.lock().unwrap();
            (locked.num_elements, locked.byte_size())
        };
        assert_eq!(
            bar_before, bar_after,
            "BAR size must be identical before and after commit"
        );
    }

    #[test]
    fn test_sync_draw_items_have_correct_positions_size() {
        // Verify that draw items carry correct positions_byte_size
        // so draw_batch knows where normals start.
        let path = SdfPath::from_string("/mesh_pos_size").unwrap();
        let mut mesh = HdStMesh::new(path);
        let registry = HdStResourceRegistry::new();

        // Triangle: 3 verts, flat normals computed automatically
        mesh.set_topology(HdStMeshTopology::from_faces(vec![3], vec![0, 1, 2]));
        mesh.set_positions(vec![0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0]);
        mesh.sync(&registry, true);
        registry.commit();

        let items = mesh.get_all_draw_items();
        assert_eq!(items.len(), 1, "sync must produce 1 draw item");

        let item = &items[0];
        let vbar = item
            .get_vertex_bar()
            .expect("draw item must have vertex_bar");
        let st_bar = vbar
            .as_any()
            .downcast_ref::<HdStBufferArrayRange>()
            .expect("must be HdStBufferArrayRange");

        let pos_size = st_bar.get_positions_byte_size();
        let total_size = st_bar.get_size();

        // 3 verts * 3 floats * 4 bytes = 36 bytes positions
        assert_eq!(pos_size, 36, "positions = 3 verts * 12 bytes");
        // positions + normals = 36 + 36 = 72
        assert_eq!(total_size, 72, "total = positions + normals");
        // Normals offset must fit in buffer
        assert!(pos_size < total_size, "normals must have space");
    }

    #[test]
    fn test_sync_positions_only_no_normals() {
        // Mesh with no triangles = no auto-normals = positions-only buffer.
        let path = SdfPath::from_string("/mesh_no_nrm").unwrap();
        let mut mesh = HdStMesh::new(path);
        let registry = HdStResourceRegistry::new();

        mesh.set_topology(HdStMeshTopology::from_faces(vec![3], vec![0, 1, 2]));
        mesh.set_positions(vec![0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0]);

        // Manually clear normals to test positions-only path
        mesh.vertex_data.normals.clear();
        mesh.triangle_indices.clear(); // no triangles = no auto-normal compute

        mesh.sync_vertices(&registry);
        registry.commit();

        let vbar = mesh.vertex_bar.as_ref().expect("vertex_bar");
        let locked = vbar.lock().unwrap();
        // Positions only: 3 verts * 12 bytes = 36
        assert_eq!(locked.byte_size(), 36, "no normals => positions only");
    }

    #[test]
    fn test_face_varying_normals_skip_smooth_normal_rebuild() {
        let path = SdfPath::from_string("/mesh_fvar_normals").unwrap();
        let mut mesh = HdStMesh::new(path);

        mesh.set_topology(HdStMeshTopology::from_faces(vec![3], vec![0, 1, 2]));
        mesh.set_positions(vec![0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0]);
        mesh.triangle_indices = vec![0, 1, 2];
        mesh.vertex_data.normals.clear();
        mesh.face_varying_primvars.normals = vec![
            0.0, 0.0, 1.0, // c0
            0.0, 0.0, 1.0, // c1
            0.0, 0.0, 1.0, // c2
        ];

        mesh.process_vertices_cpu();

        assert!(
            mesh.vertex_data.normals.is_empty(),
            "authored face-varying normals must block smooth-normal fallback"
        );
        assert_eq!(
            mesh.face_varying_primvars.normals.len(),
            9,
            "retained face-varying normals must stay intact"
        );
    }

    #[test]
    fn test_topology_without_face_varying_data_skips_fvar_index_map() {
        let path = SdfPath::from_string("/mesh_no_fvar").unwrap();
        let mut mesh = HdStMesh::new(path);

        mesh.set_topology(HdStMeshTopology::from_faces(vec![4], vec![0, 1, 2, 3]));
        mesh.set_positions(vec![
            0.0, 0.0, 0.0, //
            1.0, 0.0, 0.0, //
            1.0, 1.0, 0.0, //
            0.0, 1.0, 0.0,
        ]);

        mesh.process_topology_cpu();

        assert_eq!(mesh.triangle_indices, vec![0, 1, 2, 0, 2, 3]);
        assert!(
            mesh.triangulated_fvar_indices.is_empty(),
            "meshes without authored face-varying data must not build fvar index maps"
        );
    }

    #[test]
    fn test_face_varying_topology_recook_restarts_from_authored_positions() {
        let path = SdfPath::from_string("/mesh_fvar_recook").unwrap();
        let mut mesh = HdStMesh::new(path);

        mesh.set_topology(HdStMeshTopology::from_faces(vec![4], vec![0, 1, 2, 3]));
        mesh.set_positions(vec![
            0.0, 0.0, 0.0, //
            1.0, 0.0, 0.0, //
            1.0, 1.0, 0.0, //
            0.0, 1.0, 0.0,
        ]);
        mesh.face_varying_primvars.uvs = vec![
            0.0, 0.0, //
            1.0, 0.0, //
            1.0, 1.0, //
            0.0, 1.0,
        ];

        mesh.process_topology_cpu();
        let once = mesh.vertex_data.positions.clone();
        let expected = vec![
            0.0, 0.0, 0.0, //
            1.0, 0.0, 0.0, //
            1.0, 1.0, 0.0, //
            0.0, 0.0, 0.0, //
            1.0, 1.0, 0.0, //
            0.0, 1.0, 0.0,
        ];
        assert_eq!(once, expected, "first cook must triangulate authored positions");

        mesh.process_topology_cpu();

        assert_eq!(
            mesh.vertex_data.positions, expected,
            "topology-only recook must restart from authored positions instead of re-expanding cooked data"
        );
    }

    #[test]
    fn test_restore_authored_vertex_state_resets_topology_point_count() {
        let path = SdfPath::from_string("/mesh_restore_authored").unwrap();
        let mut mesh = HdStMesh::new(path);

        mesh.set_topology(HdStMeshTopology::from_faces(vec![4], vec![0, 1, 2, 3]));
        mesh.set_positions(vec![
            0.0, 0.0, 0.0, //
            1.0, 0.0, 0.0, //
            1.0, 1.0, 0.0, //
            0.0, 1.0, 0.0,
        ]);
        mesh.topology.set_num_points(6);

        mesh.restore_authored_vertex_state();

        assert_eq!(
            mesh.topology.get_vertex_count(),
            4,
            "restoring authored state must also restore the authored point count"
        );
    }

    #[test]
    fn test_face_varying_vertex_recook_runs_without_topology_dirty() {
        let path = SdfPath::from_string("/mesh_fvar_anim").unwrap();
        let mut mesh = HdStMesh::new(path);

        mesh.set_topology(HdStMeshTopology::from_faces(vec![4], vec![0, 1, 2, 3]));
        mesh.set_positions(vec![
            0.0, 0.0, 0.0, //
            1.0, 0.0, 0.0, //
            1.0, 1.0, 0.0, //
            0.0, 1.0, 0.0,
        ]);
        mesh.face_varying_primvars.uvs = vec![
            0.0, 0.0, //
            1.0, 0.0, //
            1.0, 1.0, //
            0.0, 1.0,
        ];

        mesh.process_topology_cpu();
        mesh.topology_dirty = false;
        mesh.vertex_dirty = false;

        mesh.set_positions(vec![
            0.0, 0.0, 1.0, //
            1.0, 0.0, 1.0, //
            1.0, 1.0, 1.0, //
            0.0, 1.0, 1.0,
        ]);
        mesh.process_cpu();

        assert_eq!(
            mesh.vertex_data.positions,
            vec![
                0.0, 0.0, 1.0, //
                1.0, 0.0, 1.0, //
                1.0, 1.0, 1.0, //
                0.0, 0.0, 1.0, //
                1.0, 1.0, 1.0, //
                0.0, 1.0, 1.0,
            ],
            "animated points must re-run the face-varying fallback even when topology stayed clean"
        );
        assert_eq!(
            mesh.triangle_indices,
            vec![0, 1, 2, 3, 4, 5],
            "fallback topology indices must stay in the cooked sequential form"
        );
    }

    #[test]
    fn test_leaving_face_varying_fallback_restores_authored_topology() {
        let path = SdfPath::from_string("/mesh_leave_fvar").unwrap();
        let mut mesh = HdStMesh::new(path);

        mesh.set_topology(HdStMeshTopology::from_faces(vec![4], vec![0, 1, 2, 3]));
        mesh.set_positions(vec![
            0.0, 0.0, 0.0, //
            1.0, 0.0, 0.0, //
            1.0, 1.0, 0.0, //
            0.0, 1.0, 0.0,
        ]);
        mesh.face_varying_primvars.uvs = vec![
            0.0, 0.0, //
            1.0, 0.0, //
            1.0, 1.0, //
            0.0, 1.0,
        ];

        mesh.process_topology_cpu();
        mesh.topology_dirty = false;
        mesh.vertex_dirty = false;

        mesh.face_varying_primvars.clear();
        mesh.set_positions(vec![
            0.0, 0.0, 2.0, //
            1.0, 0.0, 2.0, //
            1.0, 1.0, 2.0, //
            0.0, 1.0, 2.0,
        ]);
        mesh.process_cpu();

        assert_eq!(
            mesh.triangle_indices,
            vec![0, 1, 2, 0, 2, 3],
            "leaving the fallback path must restore the original triangulated topology"
        );
        assert_eq!(
            mesh.vertex_data.positions,
            vec![
                0.0, 0.0, 2.0, //
                1.0, 0.0, 2.0, //
                1.0, 1.0, 2.0, //
                0.0, 1.0, 2.0,
            ],
            "non-fvar recook must return to authored positions instead of keeping expanded buffers"
        );
    }

    #[test]
    fn test_sync_twice_reuses_bar() {
        // Syncing twice must reuse the same BAR (update, not allocate new).
        let path = SdfPath::from_string("/mesh_resync").unwrap();
        let mut mesh = HdStMesh::new(path);
        let registry = HdStResourceRegistry::new();

        mesh.set_topology(HdStMeshTopology::from_faces(vec![3], vec![0, 1, 2]));
        mesh.set_positions(vec![0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0]);
        mesh.sync(&registry, true);
        registry.commit();

        let bar_count_1 = registry.get_bar_count();

        // Re-sync with updated positions
        mesh.set_positions(vec![0.0, 0.0, 1.0, 1.0, 0.0, 1.0, 0.0, 1.0, 1.0]);
        mesh.sync(&registry, true);
        registry.commit();

        let bar_count_2 = registry.get_bar_count();
        // Should not leak BARs (same or less, not more)
        assert!(
            bar_count_2 <= bar_count_1 + 1,
            "resync should not keep allocating new BARs: {} -> {}",
            bar_count_1,
            bar_count_2
        );
    }

    #[test]
    fn test_draw_items_repr_filtering() {
        let path = SdfPath::from_string("/mesh").unwrap();
        let mut mesh = HdStMesh::new(path.clone());

        // Add draw items with different reprs using RwLock for interior mutability
        let refined_item = Arc::new(RwLock::new(HdStDrawItem::new(path.clone())));
        refined_item.write().set_repr(Token::new("refined"));

        let hull_item = Arc::new(RwLock::new(HdStDrawItem::new(path.clone())));
        hull_item.write().set_repr(Token::new("hull"));

        // For the test we use non-RwLock versions since add_draw_item expects Arc<HdStDrawItem>
        let item1 = HdStDrawItem::new(path.clone());
        item1.set_repr(Token::new("refined"));
        mesh.add_draw_item(Arc::new(item1));

        let item2 = HdStDrawItem::new(path.clone());
        item2.set_repr(Token::new("hull"));
        mesh.add_draw_item(Arc::new(item2));

        let item3 = HdStDrawItem::new(path);
        item3.set_repr(Token::new("refined"));
        mesh.add_draw_item(Arc::new(item3));

        // Filter by repr
        let refined_items = mesh.get_draw_items(&Token::new("refined"));
        assert_eq!(refined_items.len(), 2);

        let hull_items = mesh.get_draw_items(&Token::new("hull"));
        assert_eq!(hull_items.len(), 1);

        let wire_items = mesh.get_draw_items(&Token::new("wireOnSurf"));
        assert_eq!(wire_items.len(), 0);
    }
}
