// Copyright 2013 DreamWorks Animation LLC.
// Ported to Rust from OpenSubdiv 3.7.0 far/patchMap.h + patchMap.cpp

//! Quadtree-based map from (faceId, u, v) to patch handle.
//!
//! [`PatchMap`] builds a per-face quadtree over all patches in a
//! [`PatchTable`], enabling efficient lookup of the sub-patch containing
//! a given parametric location.
//!
//! Mirrors C++ `Far::PatchMap`.

use super::patch_table::{PatchTable, PatchHandle};

// ---------------------------------------------------------------------------
// QuadNode — a node in the patch quadtree
// ---------------------------------------------------------------------------

/// Child slot in a [`QuadNode`].  Packed: 1 bit isSet, 1 bit isLeaf, 30 bits index.
#[derive(Clone, Copy, Default)]
struct Child {
    is_set:  bool,
    is_leaf: bool,
    index:   u32, // 30-bit index (node or handle)
}

/// A node with 4 children in the patch quadtree.
#[derive(Clone, Copy, Default)]
struct QuadNode {
    children: [Child; 4],
}

impl QuadNode {
    /// Set all four children to the same leaf handle index.
    fn set_children(&mut self, index: u32) {
        for c in &mut self.children {
            c.is_set  = true;
            c.is_leaf = true;
            c.index   = index;
        }
    }

    /// Set a specific quadrant child.
    fn set_child(&mut self, quadrant: usize, index: u32, is_leaf: bool) {
        debug_assert!(!self.children[quadrant].is_set);
        self.children[quadrant].is_set  = true;
        self.children[quadrant].is_leaf = is_leaf;
        self.children[quadrant].index   = index;
    }
}

// ---------------------------------------------------------------------------
// PatchMap
// ---------------------------------------------------------------------------

/// Quadtree-based map from (faceId, u, v) to [`PatchHandle`].
///
/// Mirrors C++ `Far::PatchMap`.
pub struct PatchMap {
    patches_are_triangular: bool,
    min_patch_face: i32,
    max_patch_face: i32,
    max_depth:      i32,
    handles:  Vec<PatchHandle>,
    quadtree: Vec<QuadNode>,
}

impl PatchMap {
    /// Build a PatchMap from a compiled PatchTable.
    ///
    /// Mirrors C++ `Far::PatchMap::PatchMap(PatchTable const &)`.
    #[doc(alias = "PatchMap")]
    pub fn new(patch_table: &PatchTable) -> Self {
        let patches_are_triangular =
            patch_table.get_varying_patch_descriptor().get_num_control_vertices() == 3;

        let mut pm = Self {
            patches_are_triangular,
            min_patch_face: -1,
            max_patch_face: -1,
            max_depth:      0,
            handles:  Vec::new(),
            quadtree: Vec::new(),
        };

        let num_patches_total: i32 = (0..patch_table.get_num_patch_arrays())
            .map(|a| patch_table.get_num_patches(a))
            .sum();

        if num_patches_total > 0 {
            pm.init_handles(patch_table);
            pm.init_quadtree(patch_table);
        }
        pm
    }

    /// Find the patch handle for the given Ptex face and (u, v) coordinates.
    ///
    /// Returns `None` if the face is out of range or is a hole.
    /// Mirrors C++ `Far::PatchMap::FindPatch(faceIndex, u, v)`.
    #[doc(alias = "FindPatch")]
    pub fn find_patch(&self, face_id: i32, mut u: f64, mut v: f64) -> Option<&PatchHandle> {
        if face_id < self.min_patch_face || face_id > self.max_patch_face {
            return None;
        }

        let root_idx = (face_id - self.min_patch_face) as usize;
        let mut node = &self.quadtree[root_idx];

        // Root not set => hole
        if !node.children[0].is_set {
            return None;
        }

        debug_assert!(u >= 0.0 && u <= 1.0 && v >= 0.0 && v <= 1.0);

        let mut median = 0.5_f64;
        let mut tri_rotated = false;

        for _depth in 0..=self.max_depth {
            let quadrant = if self.patches_are_triangular {
                transform_uv_to_tri_quadrant(median, &mut u, &mut v, &mut tri_rotated)
            } else {
                transform_uv_to_quad_quadrant(median, &mut u, &mut v)
            };

            let child = &node.children[quadrant];
            debug_assert!(child.is_set);

            if child.is_leaf {
                return Some(&self.handles[child.index as usize]);
            } else {
                node = &self.quadtree[child.index as usize];
            }

            median *= 0.5;
        }
        None
    }

    // -----------------------------------------------------------------------
    // Construction
    // -----------------------------------------------------------------------

    fn init_handles(&mut self, pt: &PatchTable) {
        let params = pt.get_patch_param_table();
        if params.is_empty() { return; }

        self.min_patch_face = params[0].get_face_id();
        self.max_patch_face = self.min_patch_face;

        let num_arrays = pt.get_num_patch_arrays();
        let mut handle_idx = 0u32;

        for p_array in 0..num_arrays {
            let patch_size = pt.get_patch_array_descriptor(p_array)
                .get_num_control_vertices() as u32;
            let n = pt.get_num_patches(p_array);

            for j in 0..n {
                let face_id = params[handle_idx as usize].get_face_id();
                self.min_patch_face = self.min_patch_face.min(face_id);
                self.max_patch_face = self.max_patch_face.max(face_id);

                self.handles.push(PatchHandle {
                    array_index: p_array,
                    patch_index: handle_idx as i32,
                    vert_index:  (j as u32 * patch_size) as i32,
                });
                handle_idx += 1;
            }
        }
    }

    fn init_quadtree(&mut self, pt: &PatchTable) {
        let n_patch_faces = (self.max_patch_face - self.min_patch_face + 1) as usize;
        let n_handles = self.handles.len();

        self.quadtree.reserve(n_patch_faces + n_handles);
        self.quadtree.resize(n_patch_faces, QuadNode::default());

        let params = pt.get_patch_param_table();

        for handle in 0..n_handles {
            let param = &params[handle];
            let depth     = param.get_depth();
            let root_depth = if param.non_quad_root() { 1 } else { 0 };

            if depth > self.max_depth {
                self.max_depth = depth;
            }

            let node_idx = (param.get_face_id() - self.min_patch_face) as usize;

            if depth == root_depth {
                // Root-level patch: all 4 children point to this handle
                self.quadtree[node_idx].set_children(handle as u32);
                continue;
            }

            if !self.patches_are_triangular {
                // Quad: use UV bits directly to determine quadrants
                let u = param.get_u();
                let v = param.get_v();
                let mut cur = node_idx;

                for j in (root_depth + 1)..=depth {
                    let u_bit = (u >> (depth - j)) & 1;
                    let v_bit = (v >> (depth - j)) & 1;
                    let quadrant = ((v_bit << 1) | u_bit) as usize;
                    let is_leaf = j == depth;

                    cur = self.assign_leaf_or_child(cur, is_leaf, quadrant, handle as u32);
                }
            } else {
                // Tri: use an interior UV point to identify quadrants
                let mut u = 0.25_f64;
                let mut v = 0.25_f64;
                // Make a mutable copy for unnormalize
                let pp = *param;
                pp.unnormalize_triangle(&mut u, &mut v);

                let mut median = 0.5_f64;
                let mut tri_rotated = false;
                let mut cur = node_idx;

                for j in (root_depth + 1)..=depth {
                    let quadrant = transform_uv_to_tri_quadrant(
                        median, &mut u, &mut v, &mut tri_rotated,
                    );
                    let is_leaf = j == depth;
                    cur = self.assign_leaf_or_child(cur, is_leaf, quadrant, handle as u32);
                    median *= 0.5;
                }
            }
        }

        // Shrink to fit (equivalent of C++ swap trick)
        self.quadtree.shrink_to_fit();
    }

    /// Assign a leaf handle or traverse/create a child node.
    /// Returns the node index to continue from (meaningful only for non-leaf).
    fn assign_leaf_or_child(
        &mut self,
        node_idx: usize,
        is_leaf:  bool,
        quadrant: usize,
        handle:   u32,
    ) -> usize {
        if is_leaf {
            self.quadtree[node_idx].set_child(quadrant, handle, true);
            node_idx
        } else if self.quadtree[node_idx].children[quadrant].is_set {
            // Already has a child node — traverse into it
            self.quadtree[node_idx].children[quadrant].index as usize
        } else {
            // Create a new child node
            let new_idx = self.quadtree.len() as u32;
            self.quadtree.push(QuadNode::default());
            self.quadtree[node_idx].set_child(quadrant, new_idx, false);
            new_idx as usize
        }
    }
}

// ---------------------------------------------------------------------------
// UV-to-quadrant transforms (mirrors C++ static template methods)
// ---------------------------------------------------------------------------

/// Transform (u,v) into the quad quadrant that contains them. Returns quadrant 0..3.
#[inline]
fn transform_uv_to_quad_quadrant(median: f64, u: &mut f64, v: &mut f64) -> usize {
    let u_half = if *u >= median { *u -= median; 1 } else { 0 };
    let v_half = if *v >= median { *v -= median; 1 } else { 0 };
    (v_half << 1) | u_half
}

/// Transform (u,v) into the triangle quadrant that contains them. Returns quadrant 0..3.
/// Also tracks/updates the `rotated` state of the current triangle.
#[inline]
fn transform_uv_to_tri_quadrant(
    median: f64, u: &mut f64, v: &mut f64, rotated: &mut bool,
) -> usize {
    if !*rotated {
        if *u >= median { *u -= median; return 1; }
        if *v >= median { *v -= median; return 2; }
        if (*u + *v) >= median { *rotated = true; return 3; }
        0
    } else {
        if *u < median { *v -= median; return 1; }
        if *v < median { *u -= median; return 2; }
        *u -= median;
        *v -= median;
        if (*u + *v) < median { *rotated = false; return 3; }
        0
    }
}

unsafe impl Send for PatchMap {}
unsafe impl Sync for PatchMap {}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quad_quadrant_basic() {
        let mut u = 0.7;
        let mut v = 0.3;
        let q = transform_uv_to_quad_quadrant(0.5, &mut u, &mut v);
        assert_eq!(q, 1); // u >= 0.5, v < 0.5 => quadrant 1
        assert!((u - 0.2).abs() < 1e-12);
        assert!((v - 0.3).abs() < 1e-12);
    }

    #[test]
    fn tri_quadrant_non_rotated() {
        let mut u = 0.6;
        let mut v = 0.1;
        let mut rot = false;
        let q = transform_uv_to_tri_quadrant(0.5, &mut u, &mut v, &mut rot);
        assert_eq!(q, 1); // u >= median
        assert!(!rot);
    }

    #[test]
    fn empty_patch_table() {
        let pt = PatchTable::new();
        let pm = PatchMap::new(&pt);
        assert!(pm.find_patch(0, 0.5, 0.5).is_none());
    }
}
