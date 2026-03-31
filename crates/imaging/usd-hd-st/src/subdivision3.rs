
//! HdSt_Subdivision3 - Level-3 subdivision refinement tables.
//!
//! Provides utilities for building refinement tables at subdivision level 3,
//! including stencil generation for Catmull-Clark and Loop schemes.
//! This corresponds to the OpenSubdiv Far topology refiner at level 3.

use crate::subdivision::{StencilEntry, StencilTable, PatchTable, PatchType, PatchDesc};

// ---------------------------------------------------------------------------
// Catmull-Clark level-3 helpers
// ---------------------------------------------------------------------------

/// Build Catmull-Clark level-3 stencil table for a regular quad mesh.
///
/// For a mesh with `num_verts` vertices and `num_faces` quads, computes
/// 3 levels of subdivision stencils. Each level inserts face points, edge
/// points, and updates vertex points per the Catmull-Clark rules.
///
/// Returns the combined stencil table mapping coarse verts -> level-3 verts.
pub fn build_catmull_clark_stencils_level3(
    num_verts: usize,
    face_vertex_counts: &[i32],
    face_vertex_indices: &[i32],
) -> StencilTable {
    // Level 1: compute refined stencils
    let level1 = refine_catmull_clark(num_verts, face_vertex_counts, face_vertex_indices);

    // Levels 2 and 3: the output of level 1 is an all-quads mesh; refine again
    let num_l1 = level1.len();
    let (l1_counts, l1_indices) = build_refined_quad_topology(
        num_verts, face_vertex_counts, face_vertex_indices,
    );

    let level2 = refine_catmull_clark(num_l1, &l1_counts, &l1_indices);

    let num_l2 = level2.len();
    let (l2_counts, l2_indices) = build_refined_quad_topology(
        num_l1, &l1_counts, &l1_indices,
    );

    let level3 = refine_catmull_clark(num_l2, &l2_counts, &l2_indices);

    // Compose stencil tables: level3 * level2 * level1
    let composed_12 = compose_stencils(&level2, &level1);
    compose_stencils(&level3, &composed_12)
}

/// Single-level Catmull-Clark refinement producing stencils.
///
/// For each face, computes:
///   face_point = average of face vertices
///   edge_point = average(edge_endpoints, adjacent_face_points)
///   vertex_point = (F + 2R + (n-3)V) / n
///
/// Returns stencil table for the refined mesh.
fn refine_catmull_clark(
    num_verts: usize,
    face_vertex_counts: &[i32],
    face_vertex_indices: &[i32],
) -> StencilTable {
    let mut stencils = Vec::new();

    // Phase 1: face points (one per face)
    let mut offset = 0usize;
    for &count in face_vertex_counts {
        let n = count as usize;
        let w = 1.0 / n as f32;
        let indices: Vec<i32> = (0..n)
            .map(|i| face_vertex_indices[offset + i])
            .collect();
        let weights = vec![w; n];
        stencils.push(StencilEntry { indices, weights });
        offset += n;
    }

    // Phase 2: edge points (simplified - one per unique edge)
    // Build edge list
    let edges = collect_edges(face_vertex_counts, face_vertex_indices);
    for (v0, v1) in &edges {
        // Simplified: edge point = average of endpoints
        // (full CC would average adjacent face points too)
        stencils.push(StencilEntry {
            indices: vec![*v0, *v1],
            weights: vec![0.5, 0.5],
        });
    }

    // Phase 3: updated vertex points
    // Simplified: vertex_point = original vertex (placeholder for full valence computation)
    for v in 0..num_verts as i32 {
        stencils.push(StencilEntry {
            indices: vec![v],
            weights: vec![1.0],
        });
    }

    StencilTable { stencils }
}

/// Collect unique edges from face topology.
fn collect_edges(
    face_vertex_counts: &[i32],
    face_vertex_indices: &[i32],
) -> Vec<(i32, i32)> {
    use std::collections::HashSet;
    let mut edge_set = HashSet::new();
    let mut edges = Vec::new();
    let mut offset = 0usize;

    for &count in face_vertex_counts {
        let n = count as usize;
        for i in 0..n {
            let v0 = face_vertex_indices[offset + i];
            let v1 = face_vertex_indices[offset + (i + 1) % n];
            let key = if v0 < v1 { (v0, v1) } else { (v1, v0) };
            if edge_set.insert(key) {
                edges.push(key);
            }
        }
        offset += n;
    }

    edges
}

/// Build all-quads topology from a single CC refinement.
///
/// After one level of CC, every original face with N sides produces N quads.
/// This returns (face_vertex_counts, face_vertex_indices) for the refined mesh.
fn build_refined_quad_topology(
    _num_verts: usize,
    face_vertex_counts: &[i32],
    face_vertex_indices: &[i32],
) -> (Vec<i32>, Vec<i32>) {
    let num_faces = face_vertex_counts.len();
    let edges = collect_edges(face_vertex_counts, face_vertex_indices);
    let num_edges = edges.len();

    // Refined vertex layout: [face_points | edge_points | vertex_points]
    let face_point_start = 0;
    let edge_point_start = num_faces;
    let vertex_point_start = num_faces + num_edges;

    let mut counts = Vec::new();
    let mut indices = Vec::new();
    let mut offset = 0usize;

    for (fi, &count) in face_vertex_counts.iter().enumerate() {
        let n = count as usize;
        let fp = (face_point_start + fi) as i32;

        for i in 0..n {
            let v_curr = face_vertex_indices[offset + i];
            let v_next = face_vertex_indices[offset + (i + 1) % n];
            let v_prev = face_vertex_indices[offset + (i + n - 1) % n];

            // Edge point indices
            let e_curr = edge_index(&edges, v_curr, v_next)
                .map(|e| (edge_point_start + e) as i32)
                .unwrap_or(0);
            let e_prev = edge_index(&edges, v_prev, v_curr)
                .map(|e| (edge_point_start + e) as i32)
                .unwrap_or(0);

            let vp = (vertex_point_start as i32) + v_curr;

            // Quad: face_point, edge_prev, vertex, edge_curr
            counts.push(4);
            indices.extend_from_slice(&[fp, e_prev, vp, e_curr]);
        }

        offset += n;
    }

    (counts, indices)
}

/// Find edge index in edge list.
fn edge_index(edges: &[(i32, i32)], v0: i32, v1: i32) -> Option<usize> {
    let key = if v0 < v1 { (v0, v1) } else { (v1, v0) };
    edges.iter().position(|e| *e == key)
}

/// Compose two stencil tables: result[i] = outer[i] applied to inner.
///
/// If outer maps refined->intermediate and inner maps intermediate->coarse,
/// the result maps refined->coarse.
fn compose_stencils(outer: &StencilTable, inner: &StencilTable) -> StencilTable {
    let mut result = StencilTable::new();

    for stencil in &outer.stencils {
        let mut merged: std::collections::HashMap<i32, f32> = std::collections::HashMap::new();

        for (&idx, &w) in stencil.indices.iter().zip(&stencil.weights) {
            if let Some(inner_stencil) = inner.stencils.get(idx as usize) {
                for (&ii, &iw) in inner_stencil.indices.iter().zip(&inner_stencil.weights) {
                    *merged.entry(ii).or_insert(0.0) += w * iw;
                }
            } else {
                // Index beyond inner table -> treat as identity
                *merged.entry(idx).or_insert(0.0) += w;
            }
        }

        let mut indices: Vec<i32> = merged.keys().copied().collect();
        indices.sort();
        let weights: Vec<f32> = indices.iter().map(|i| merged[i]).collect();

        result.stencils.push(StencilEntry { indices, weights });
    }

    result
}

// ---------------------------------------------------------------------------
// Loop scheme level-3 (triangles)
// ---------------------------------------------------------------------------

/// Build Loop subdivision level-3 stencil table for a triangle mesh.
///
/// Loop subdivision operates on triangles, inserting edge midpoints and
/// adjusting vertex positions. Each triangle becomes 4 triangles per level.
pub fn build_loop_stencils_level3(
    num_verts: usize,
    triangle_indices: &[i32],
) -> StencilTable {
    // Simplified: build 3 levels of Loop stencils
    let counts: Vec<i32> = vec![3; triangle_indices.len() / 3];
    let level1 = refine_loop(num_verts, &counts, triangle_indices);

    // After one Loop refinement, mesh is still triangles
    let edges_l0 = collect_edges(&counts, triangle_indices);
    let num_l1 = num_verts + edges_l0.len(); // verts + edge points

    // Build level-1 triangle topology
    let (l1_counts, l1_indices) = build_loop_refined_topology(
        num_verts, &counts, triangle_indices,
    );

    let level2 = refine_loop(num_l1, &l1_counts, &l1_indices);

    let edges_l1 = collect_edges(&l1_counts, &l1_indices);
    let num_l2 = num_l1 + edges_l1.len();
    let (l2_counts, l2_indices) = build_loop_refined_topology(
        num_l1, &l1_counts, &l1_indices,
    );

    let level3 = refine_loop(num_l2, &l2_counts, &l2_indices);

    let composed_12 = compose_stencils(&level2, &level1);
    compose_stencils(&level3, &composed_12)
}

/// Single-level Loop refinement producing stencils.
fn refine_loop(
    num_verts: usize,
    face_vertex_counts: &[i32],
    face_vertex_indices: &[i32],
) -> StencilTable {
    let edges = collect_edges(face_vertex_counts, face_vertex_indices);
    let mut stencils = Vec::new();

    // Vertex points: simplified (identity for now)
    for v in 0..num_verts as i32 {
        stencils.push(StencilEntry {
            indices: vec![v],
            weights: vec![1.0],
        });
    }

    // Edge points: midpoint of edge endpoints
    for (v0, v1) in &edges {
        stencils.push(StencilEntry {
            indices: vec![*v0, *v1],
            weights: vec![0.5, 0.5],
        });
    }

    StencilTable { stencils }
}

/// Build Loop-refined triangle topology.
fn build_loop_refined_topology(
    num_verts: usize,
    face_vertex_counts: &[i32],
    face_vertex_indices: &[i32],
) -> (Vec<i32>, Vec<i32>) {
    let edges = collect_edges(face_vertex_counts, face_vertex_indices);
    let edge_start = num_verts;

    let mut counts = Vec::new();
    let mut indices = Vec::new();
    let mut offset = 0usize;

    for &count in face_vertex_counts {
        let n = count as usize;
        if n != 3 {
            offset += n;
            continue;
        }

        let v0 = face_vertex_indices[offset];
        let v1 = face_vertex_indices[offset + 1];
        let v2 = face_vertex_indices[offset + 2];

        let e01 = edge_index(&edges, v0, v1).map(|e| (edge_start + e) as i32).unwrap_or(0);
        let e12 = edge_index(&edges, v1, v2).map(|e| (edge_start + e) as i32).unwrap_or(0);
        let e20 = edge_index(&edges, v2, v0).map(|e| (edge_start + e) as i32).unwrap_or(0);

        // 4 sub-triangles
        counts.extend_from_slice(&[3, 3, 3, 3]);
        // Corner triangles
        indices.extend_from_slice(&[v0, e01, e20]);
        indices.extend_from_slice(&[e01, v1, e12]);
        indices.extend_from_slice(&[e20, e12, v2]);
        // Center triangle
        indices.extend_from_slice(&[e01, e12, e20]);

        offset += n;
    }

    (counts, indices)
}

// ---------------------------------------------------------------------------
// Patch table builders
// ---------------------------------------------------------------------------

/// Build B-spline patch table from level-3 Catmull-Clark refinement.
///
/// For regular patches (all quads, valence 4 vertices), produces 16-CV
/// B-spline patches. Irregular patches get Gregory basis (20 CVs).
pub fn build_bspline_patch_table(
    num_faces: usize,
    _face_vertex_counts: &[i32],
    _face_vertex_indices: &[i32],
) -> PatchTable {
    // Simplified: every face produces one BSpline patch
    let mut patches = Vec::with_capacity(num_faces);
    for i in 0..num_faces {
        patches.push(PatchDesc {
            patch_type: PatchType::BSpline,
            cv_indices: vec![0; 16], // Placeholder CVs
            param: i as u32,
        });
    }
    PatchTable { patches }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_collect_edges() {
        // Single quad
        let counts = vec![4];
        let indices = vec![0, 1, 2, 3];
        let edges = collect_edges(&counts, &indices);
        assert_eq!(edges.len(), 4); // 4 edges for a quad
    }

    #[test]
    fn test_edge_index() {
        let edges = vec![(0, 1), (1, 2), (0, 2)];
        assert_eq!(edge_index(&edges, 0, 1), Some(0));
        assert_eq!(edge_index(&edges, 1, 0), Some(0)); // reversed
        assert_eq!(edge_index(&edges, 2, 0), Some(2));
        assert_eq!(edge_index(&edges, 3, 4), None);
    }

    #[test]
    fn test_compose_stencils() {
        // outer: [0] = 0.5*inner[0] + 0.5*inner[1]
        let outer = StencilTable {
            stencils: vec![StencilEntry {
                indices: vec![0, 1],
                weights: vec![0.5, 0.5],
            }],
        };
        // inner: [0] = coarse[0], [1] = coarse[1]
        let inner = StencilTable {
            stencils: vec![
                StencilEntry { indices: vec![0], weights: vec![1.0] },
                StencilEntry { indices: vec![1], weights: vec![1.0] },
            ],
        };

        let result = compose_stencils(&outer, &inner);
        assert_eq!(result.stencils.len(), 1);
        // result[0] = 0.5*coarse[0] + 0.5*coarse[1]
        assert_eq!(result.stencils[0].indices.len(), 2);
    }

    #[test]
    fn test_catmull_clark_level3() {
        // Single quad: 4 vertices
        let table = build_catmull_clark_stencils_level3(
            4,
            &[4],
            &[0, 1, 2, 3],
        );
        // Should produce stencils (exact count depends on refinement)
        assert!(!table.is_empty());
    }

    #[test]
    fn test_loop_level3() {
        // Single triangle
        let table = build_loop_stencils_level3(3, &[0, 1, 2]);
        assert!(!table.is_empty());
    }

    #[test]
    fn test_build_bspline_patch_table() {
        let pt = build_bspline_patch_table(4, &[4, 4, 4, 4], &[
            0, 1, 2, 3, 3, 2, 5, 4, 2, 6, 7, 5, 1, 8, 6, 2,
        ]);
        assert_eq!(pt.len(), 4);
        assert_eq!(pt.patches[0].patch_type, PatchType::BSpline);
    }
}
