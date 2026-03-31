
//! HdSt_MeshTopology - Storm mesh topology with triangulation, quadrangulation,
//! and subdivision support.
//!
//! Full mesh topology management including face/vertex counts, hole indices,
//! geom subsets, and computation of triangulation/quadrangulation tables.
//! See pxr/imaging/hdSt/meshTopology.h for C++ reference.

use std::sync::Arc;
use usd_sdf::Path as SdfPath;
use usd_tf::Token;

// ---------------------------------------------------------------------------
// Enums
// ---------------------------------------------------------------------------

/// How subdivision mesh topology is refined.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RefineMode {
    /// Uniform refinement (all faces subdivided equally)
    #[default]
    Uniform = 0,
    /// Patch-based refinement (for hardware tessellation)
    Patches,
}

/// Whether quads are triangulated or kept as quads.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum QuadsMode {
    /// Quads are triangulated into two triangles
    Triangulated = 0,
    /// Quads remain as quads
    #[default]
    Untriangulated,
}

/// Interpolation type used in refinement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Interpolation {
    /// Per-vertex interpolation
    #[default]
    Vertex,
    /// Varying interpolation (linear across patch)
    Varying,
    /// Face-varying interpolation (independent per face-vertex)
    FaceVarying,
}

// ---------------------------------------------------------------------------
// QuadInfo
// ---------------------------------------------------------------------------

/// Quadrangulation info for Catmull-Clark subdivision.
///
/// Stores per-face vertex counts and accumulated vertex offsets for
/// converting arbitrary polygons to quads by inserting face centers.
#[derive(Debug, Clone, Default)]
pub struct HdQuadInfo {
    /// Number of vertices per face in the original mesh
    pub verts_per_face: Vec<i32>,
    /// For each non-quad/non-tri face, the index where new center points start
    pub point_indices: Vec<i32>,
    /// Total number of quad points after quadrangulation
    pub num_points: usize,
    /// Maximum valence in the mesh
    pub max_num_verts_per_face: i32,
}

impl HdQuadInfo {
    /// Create empty quad info.
    pub fn new() -> Self {
        Self::default()
    }

    /// Check if quad info is valid (has been computed).
    pub fn is_valid(&self) -> bool {
        !self.verts_per_face.is_empty()
    }

    /// Compute quad info from face vertex counts.
    ///
    /// For faces with more than 4 vertices, a center point is inserted
    /// and the face is split into quads. Triangles are handled by
    /// inserting a center and producing 3 quads.
    pub fn compute(face_vertex_counts: &[i32]) -> Self {
        let mut info = Self::new();
        let mut num_added = 0i32;
        let mut max_verts = 0i32;

        for &count in face_vertex_counts {
            info.verts_per_face.push(count);
            if count > max_verts {
                max_verts = count;
            }

            if count != 4 {
                // Non-quad faces get a center point
                info.point_indices.push(num_added);
                num_added += 1;
            } else {
                // Quads don't need extra points
                info.point_indices.push(-1);
            }
        }

        // Total points = original vertices + newly inserted center points
        info.max_num_verts_per_face = max_verts;
        info.num_points = num_added as usize;
        info
    }
}

// ---------------------------------------------------------------------------
// GeomSubset
// ---------------------------------------------------------------------------

/// A geometric subset of a mesh, referencing a material and face indices.
#[derive(Debug, Clone)]
pub struct HdGeomSubset {
    /// Subset type (typically "materialBind")
    pub subset_type: Token,
    /// Unique id for this subset
    pub id: SdfPath,
    /// Material path bound to this subset
    pub material_id: SdfPath,
    /// Face indices belonging to this subset
    pub indices: Vec<i32>,
}

impl HdGeomSubset {
    /// Create a new geom subset.
    pub fn new(id: SdfPath, material_id: SdfPath, indices: Vec<i32>) -> Self {
        Self {
            subset_type: Token::new("materialBind"),
            id,
            material_id,
            indices,
        }
    }
}

// ---------------------------------------------------------------------------
// HdStMeshTopology (full Storm version)
// ---------------------------------------------------------------------------

/// Storm mesh topology with full triangulation/quadrangulation/subdivision.
///
/// This is the extended Storm topology that wraps HdMeshTopology with:
/// - Triangulation tables and primitive param buffers
/// - Quadrangulation tables for Catmull-Clark
/// - OpenSubdiv refinement (stencil/patch tables)
/// - Geom subset management
/// - Face-varying topology channels
#[derive(Debug, Clone)]
pub struct HdStMeshTopology {
    // -- Base topology data --
    /// Number of vertices per face
    pub face_vertex_counts: Vec<i32>,
    /// Vertex indices for all faces (flattened)
    pub face_vertex_indices: Vec<i32>,
    /// Indices of faces that are holes
    pub hole_indices: Vec<i32>,
    /// Orientation (true = right-handed)
    pub orientation_right_handed: bool,
    /// Subdivision scheme token (catmullClark, loop, bilinear, none)
    pub subdivision_scheme: Token,
    /// Geom subsets
    pub geom_subsets: Vec<HdGeomSubset>,

    // -- Refinement --
    /// Subdivision refinement level
    refine_level: i32,
    /// Refinement mode
    refine_mode: RefineMode,
    /// Quads mode
    quads_mode: QuadsMode,

    // -- Quadrangulation --
    /// Quadrangulation info (computed lazily)
    quad_info: Option<HdQuadInfo>,

    // -- Face-varying --
    /// Per-channel face-varying topology indices
    fvar_topologies: Vec<Vec<i32>>,

    // -- Geom subset internals --
    /// Faces not in any geom subset (computed by sanitize_geom_subsets)
    non_subset_faces: Option<Vec<i32>>,
}

impl HdStMeshTopology {
    /// Create from base topology data.
    pub fn new(
        face_vertex_counts: Vec<i32>,
        face_vertex_indices: Vec<i32>,
        refine_level: i32,
        refine_mode: RefineMode,
        quads_mode: QuadsMode,
    ) -> Self {
        Self {
            face_vertex_counts,
            face_vertex_indices,
            hole_indices: Vec::new(),
            orientation_right_handed: true,
            subdivision_scheme: Token::new("none"),
            geom_subsets: Vec::new(),
            refine_level,
            refine_mode,
            quads_mode,
            quad_info: None,
            fvar_topologies: Vec::new(),
            non_subset_faces: None,
        }
    }

    /// Convenience ctor from just face data (no subdivision).
    pub fn from_faces(counts: Vec<i32>, indices: Vec<i32>) -> Self {
        Self::new(
            counts,
            indices,
            0,
            RefineMode::Uniform,
            QuadsMode::Untriangulated,
        )
    }

    /// Get face count.
    pub fn get_face_count(&self) -> usize {
        self.face_vertex_counts.len()
    }

    /// Compute total number of face-vertices.
    pub fn get_num_face_vertices(&self) -> usize {
        self.face_vertex_counts.iter().map(|&c| c as usize).sum()
    }

    /// Compute the max vertex index +1 (i.e. needed vertex count).
    pub fn get_num_points(&self) -> usize {
        self.face_vertex_indices
            .iter()
            .map(|&i| i as usize)
            .max()
            .map(|m| m + 1)
            .unwrap_or(0)
    }

    /// Get refine level.
    pub fn get_refine_level(&self) -> i32 {
        self.refine_level
    }

    /// Get refine mode.
    pub fn get_refine_mode(&self) -> RefineMode {
        self.refine_mode
    }

    /// Get quads mode.
    pub fn get_quads_mode(&self) -> QuadsMode {
        self.quads_mode
    }

    /// Whether quads should be triangulated.
    pub fn triangulate_quads(&self) -> bool {
        self.quads_mode == QuadsMode::Triangulated
    }

    // -- Triangulation --

    /// Compute triangle index buffer and primitive params.
    ///
    /// Returns (triangle_indices, primitive_params) where each primitive param
    /// encodes the original coarse face index for that triangle.
    pub fn compute_triangle_indices(&self) -> (Vec<u32>, Vec<i32>) {
        let mut tri_indices = Vec::new();
        let mut prim_params = Vec::new();
        let mut offset = 0usize;

        for (face_idx, &count) in self.face_vertex_counts.iter().enumerate() {
            let count = count as usize;

            // Skip holes
            if self.hole_indices.contains(&(face_idx as i32)) {
                offset += count;
                continue;
            }

            if count < 3 {
                offset += count;
                continue;
            }

            // Fan triangulation from first vertex
            let v0 = self.face_vertex_indices[offset] as u32;
            for i in 1..count - 1 {
                let v1 = self.face_vertex_indices[offset + i] as u32;
                let v2 = self.face_vertex_indices[offset + i + 1] as u32;

                if self.orientation_right_handed {
                    tri_indices.extend_from_slice(&[v0, v1, v2]);
                } else {
                    tri_indices.extend_from_slice(&[v0, v2, v1]);
                }
                prim_params.push(face_idx as i32);
            }

            offset += count;
        }

        (tri_indices, prim_params)
    }

    /// Compute triangle edge indices for wireframe rendering.
    ///
    /// Each triangle stores which of its 3 edges are "real" polygon edges
    /// (vs internal fan edges), encoded as a bitmask.
    pub fn compute_triangle_edge_indices(&self) -> Vec<u8> {
        let mut edge_flags = Vec::new();

        for (face_idx, &count) in self.face_vertex_counts.iter().enumerate() {
            let count = count as usize;

            if self.hole_indices.contains(&(face_idx as i32)) {
                continue;
            }
            if count < 3 {
                continue;
            }

            for i in 1..count - 1 {
                let mut flags = 0u8;
                // Edge 0 (v0->v1): real only for first triangle in fan
                if i == 1 {
                    flags |= 1;
                }
                // Edge 1 (v1->v2): always a real polygon edge
                flags |= 2;
                // Edge 2 (v2->v0): real only for last triangle in fan
                if i == count - 2 {
                    flags |= 4;
                }
                edge_flags.push(flags);
            }
        }

        edge_flags
    }

    // -- Quadrangulation --

    /// Set quad info (takes ownership).
    pub fn set_quad_info(&mut self, info: HdQuadInfo) {
        self.quad_info = Some(info);
    }

    /// Get quad info reference.
    pub fn get_quad_info(&self) -> Option<&HdQuadInfo> {
        self.quad_info.as_ref()
    }

    /// Compute and store quad info from current topology.
    pub fn compute_quad_info(&mut self) {
        let info = HdQuadInfo::compute(&self.face_vertex_counts);
        self.quad_info = Some(info);
    }

    /// Compute quad index buffer.
    ///
    /// For Catmull-Clark subdivision: non-quad faces get a center point inserted,
    /// producing quads. Existing quads pass through unchanged.
    /// Returns (quad_indices_4_per_quad, primitive_params).
    pub fn compute_quad_indices(&self) -> (Vec<u32>, Vec<i32>) {
        let mut quad_indices = Vec::new();
        let mut prim_params = Vec::new();
        let mut offset = 0usize;
        let num_orig_points = self.get_num_points();

        // Need quad info for center point indexing
        let quad_info = match &self.quad_info {
            Some(qi) => qi,
            None => return (quad_indices, prim_params),
        };

        let mut center_idx = num_orig_points as u32;

        for (face_idx, &count) in self.face_vertex_counts.iter().enumerate() {
            let count = count as usize;

            if self.hole_indices.contains(&(face_idx as i32)) {
                if quad_info
                    .point_indices
                    .get(face_idx)
                    .map_or(false, |&v| v >= 0)
                {
                    center_idx += 1;
                }
                offset += count;
                continue;
            }

            if count == 4 {
                // Quad - pass through
                let v0 = self.face_vertex_indices[offset] as u32;
                let v1 = self.face_vertex_indices[offset + 1] as u32;
                let v2 = self.face_vertex_indices[offset + 2] as u32;
                let v3 = self.face_vertex_indices[offset + 3] as u32;
                quad_indices.extend_from_slice(&[v0, v1, v2, v3]);
                prim_params.push(face_idx as i32);
            } else if count >= 3 {
                // Non-quad: insert center, create quads
                let ci = center_idx;
                center_idx += 1;

                for i in 0..count {
                    let v0 = self.face_vertex_indices[offset + i] as u32;
                    let v1 = self.face_vertex_indices[offset + (i + 1) % count] as u32;
                    // Quad: v0, v1, center, prev_v
                    let prev = self.face_vertex_indices[offset + (i + count - 1) % count] as u32;
                    quad_indices.extend_from_slice(&[prev, v0, v1, ci]);
                    prim_params.push(face_idx as i32);
                }
            }

            offset += count;
        }

        (quad_indices, prim_params)
    }

    // -- Points index --

    /// Compute point indices buffer (just sequential 0..N).
    pub fn compute_points_index(&self) -> Vec<u32> {
        (0..self.get_num_points() as u32).collect()
    }

    // -- Subdivision queries --

    /// Whether subdivision on this mesh produces triangles (Loop scheme).
    pub fn refines_to_triangles(&self) -> bool {
        self.subdivision_scheme == "loop"
    }

    /// Whether subdivision produces B-spline patches (catmullClark).
    pub fn refines_to_bspline_patches(&self) -> bool {
        self.subdivision_scheme == "catmullClark"
    }

    /// Whether subdivision produces box-spline triangle patches (loop).
    pub fn refines_to_box_spline_triangle_patches(&self) -> bool {
        self.subdivision_scheme == "loop"
    }

    // -- Geom subsets --

    /// Sanitize geom subsets: remove empty ones, compute non_subset_faces.
    pub fn sanitize_geom_subsets(&mut self) {
        // Remove subsets with empty indices or empty material id
        self.geom_subsets
            .retain(|s| !s.indices.is_empty() && !s.material_id.is_empty());

        if self.geom_subsets.is_empty() {
            self.non_subset_faces = None;
            return;
        }

        // Collect all faces referenced by any subset
        let face_count = self.get_face_count();
        let mut in_subset = vec![false; face_count];
        for subset in &self.geom_subsets {
            for &idx in &subset.indices {
                if (idx as usize) < face_count {
                    in_subset[idx as usize] = true;
                }
            }
        }

        // Non-subset faces are those not in any subset
        let non_subset: Vec<i32> = (0..face_count as i32)
            .filter(|&i| !in_subset[i as usize])
            .collect();

        self.non_subset_faces = if non_subset.is_empty() {
            None
        } else {
            Some(non_subset)
        };
    }

    /// Get faces not covered by any geom subset.
    pub fn get_non_subset_faces(&self) -> Option<&[i32]> {
        self.non_subset_faces.as_deref()
    }

    // -- Face-varying --

    /// Set face-varying topologies for each channel.
    pub fn set_fvar_topologies(&mut self, topologies: Vec<Vec<i32>>) {
        self.fvar_topologies = topologies;
    }

    /// Get face-varying topologies.
    pub fn get_fvar_topologies(&self) -> &[Vec<i32>] {
        &self.fvar_topologies
    }

    // -- Equality --

    /// Check equality (topology data only, not computed tables).
    pub fn topology_eq(&self, other: &Self) -> bool {
        self.face_vertex_counts == other.face_vertex_counts
            && self.face_vertex_indices == other.face_vertex_indices
            && self.hole_indices == other.hole_indices
            && self.orientation_right_handed == other.orientation_right_handed
            && self.subdivision_scheme == other.subdivision_scheme
            && self.refine_level == other.refine_level
    }
}

/// Shared pointer to Storm mesh topology.
pub type HdStMeshTopologySharedPtr = Arc<HdStMeshTopology>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_topology() {
        let topo = HdStMeshTopology::from_faces(vec![4, 3], vec![0, 1, 2, 3, 4, 5, 6]);
        assert_eq!(topo.get_face_count(), 2);
        assert_eq!(topo.get_num_face_vertices(), 7);
        assert_eq!(topo.get_num_points(), 7);
    }

    #[test]
    fn test_triangulate() {
        // Single quad
        let topo = HdStMeshTopology::from_faces(vec![4], vec![0, 1, 2, 3]);
        let (tri, params) = topo.compute_triangle_indices();
        assert_eq!(tri.len(), 6); // 2 triangles * 3 indices
        assert_eq!(params.len(), 2); // 2 triangles
        assert_eq!(params[0], 0); // both from face 0
        assert_eq!(params[1], 0);
    }

    #[test]
    fn test_triangulate_with_holes() {
        let mut topo = HdStMeshTopology::from_faces(vec![4, 3], vec![0, 1, 2, 3, 4, 5, 6]);
        topo.hole_indices = vec![0]; // First face is a hole
        let (tri, params) = topo.compute_triangle_indices();
        assert_eq!(tri.len(), 3); // Only 1 triangle from face 1
        assert_eq!(params[0], 1); // From face 1
    }

    #[test]
    fn test_edge_indices() {
        // Single pentagon -> 3 triangles via fan
        let topo = HdStMeshTopology::from_faces(vec![5], vec![0, 1, 2, 3, 4]);
        let edges = topo.compute_triangle_edge_indices();
        assert_eq!(edges.len(), 3);
        // First triangle: edge0 real (1), edge1 real (2) = 3
        assert_eq!(edges[0], 0b011);
        // Middle triangle: edge1 real (2) = 2
        assert_eq!(edges[1], 0b010);
        // Last triangle: edge1 real (2), edge2 real (4) = 6
        assert_eq!(edges[2], 0b110);
    }

    #[test]
    fn test_quad_info() {
        let info = HdQuadInfo::compute(&[4, 3, 5]);
        assert_eq!(info.verts_per_face, vec![4, 3, 5]);
        assert_eq!(info.point_indices[0], -1); // quad, no center
        assert!(info.point_indices[1] >= 0); // tri needs center
        assert!(info.point_indices[2] >= 0); // pent needs center
        assert_eq!(info.num_points, 2); // 2 center points added
        assert_eq!(info.max_num_verts_per_face, 5);
    }

    #[test]
    fn test_points_index() {
        let topo = HdStMeshTopology::from_faces(vec![3], vec![0, 1, 2]);
        let pts = topo.compute_points_index();
        assert_eq!(pts, vec![0, 1, 2]);
    }

    #[test]
    fn test_geom_subsets() {
        let mut topo = HdStMeshTopology::from_faces(
            vec![4, 4, 4, 4],
            vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15],
        );

        let mat = SdfPath::from_string("/mat").unwrap();
        topo.geom_subsets.push(HdGeomSubset::new(
            SdfPath::from_string("/subset0").unwrap(),
            mat.clone(),
            vec![0, 2],
        ));

        topo.sanitize_geom_subsets();
        let non_subset = topo.get_non_subset_faces().unwrap();
        assert_eq!(non_subset, &[1, 3]);
    }

    #[test]
    fn test_subdivision_queries() {
        let mut topo = HdStMeshTopology::from_faces(vec![4], vec![0, 1, 2, 3]);
        topo.subdivision_scheme = Token::new("catmullClark");
        assert!(topo.refines_to_bspline_patches());
        assert!(!topo.refines_to_triangles());

        topo.subdivision_scheme = Token::new("loop");
        assert!(topo.refines_to_triangles());
        assert!(topo.refines_to_box_spline_triangle_patches());
    }
}
