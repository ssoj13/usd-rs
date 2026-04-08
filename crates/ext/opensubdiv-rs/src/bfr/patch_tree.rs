//! PatchTree — hierarchical collection of parametric patches for one face.
//!
//! Mirrors `Bfr::PatchTree` from `patchTree.h/cpp`.
//! Construction is exclusively through `PatchTreeBuilder` (not exposed here).

use crate::far::{PatchParam, PatchType};

// ---------------------------------------------------------------------------
// TreeNode
// ---------------------------------------------------------------------------

/// A node of the quad-tree used to locate patches containing a UV point.
///
/// Mirrors `Bfr::PatchTree::TreeNode`.
#[derive(Clone, Debug)]
pub struct TreeNode {
    /// Index of the patch at this node (-1 = none).
    pub patch_index: i32,
    pub children: [TreeChild; 4],
}

/// A single child slot of a `TreeNode`.
#[derive(Clone, Copy, Debug, Default)]
pub struct TreeChild {
    pub is_set: bool,
    pub is_leaf: bool,
    /// 28-bit index (patch index if leaf, node index otherwise).
    pub index: u32,
}

impl TreeChild {
    pub fn set_index(&mut self, idx: i32) {
        self.index = (idx as u32) & 0x0fff_ffff;
    }
    pub fn get_index(self) -> i32 {
        self.index as i32
    }
}

impl Default for TreeNode {
    fn default() -> Self {
        TreeNode {
            patch_index: -1,
            children: [TreeChild::default(); 4],
        }
    }
}

impl TreeNode {
    /// Point all four children at the same leaf patch.
    pub fn set_children(&mut self, index: i32) {
        for c in self.children.iter_mut() {
            c.is_set = true;
            c.is_leaf = true;
            c.set_index(index);
        }
    }

    /// Set a single child in `quadrant` to point at `index`.
    pub fn set_child(&mut self, quadrant: usize, index: i32, is_leaf: bool) {
        debug_assert!(!self.children[quadrant].is_set);
        let c = &mut self.children[quadrant];
        c.is_set = true;
        c.is_leaf = is_leaf;
        c.set_index(index);
    }
}

// ---------------------------------------------------------------------------
// PatchTree
// ---------------------------------------------------------------------------

/// Hierarchical collection of parametric patches representing the limit
/// surface of a single irregular face.
///
/// Mirrors `Bfr::PatchTree`.
///
/// Note: several fields are written by `PatchTreeBuilder` but read during
/// evaluation or query methods.  The `dead_code` allow covers fields that
/// the Rust compiler marks as written-only because the builder integration
/// with `Far::TopologyRefiner` is not yet wired at the call-site level.
#[allow(dead_code)]
#[derive(Debug)]
pub struct PatchTree {
    // Configuration:
    pub(crate) use_double_precision: bool,
    pub(crate) patches_include_non_leaf: bool,
    pub(crate) patches_are_triangular: bool,

    pub(crate) reg_patch_type: PatchType,
    pub(crate) irreg_patch_type: PatchType,
    pub(crate) reg_patch_size: i32,
    pub(crate) irreg_patch_size: i32,
    pub(crate) patch_point_stride: i32,

    // Topology inventory:
    pub(crate) num_sub_faces: i32,
    pub(crate) num_control_points: i32,
    pub(crate) num_refined_points: i32,
    pub(crate) num_sub_patch_points: i32,
    pub(crate) num_irreg_patches: i32,

    // Patch data:
    pub(crate) patch_points: Vec<i32>,
    pub(crate) patch_params: Vec<PatchParam>,

    // Quad-tree:
    pub(crate) tree_nodes: Vec<TreeNode>,
    pub(crate) tree_depth: i32,

    // Stencil matrices (single or double precision):
    pub(crate) stencil_matrix_f32: Vec<f32>,
    pub(crate) stencil_matrix_f64: Vec<f64>,
}

impl PatchTree {
    /// Create a default, empty PatchTree (builder use only).
    pub(crate) fn new() -> Self {
        PatchTree {
            use_double_precision: false,
            patches_include_non_leaf: false,
            patches_are_triangular: false,

            reg_patch_type: PatchType::NonPatch,
            irreg_patch_type: PatchType::NonPatch,
            reg_patch_size: 0,
            irreg_patch_size: 0,
            patch_point_stride: 0,

            num_sub_faces: 0,
            num_control_points: 0,
            num_refined_points: 0,
            num_sub_patch_points: 0,
            num_irreg_patches: 0,

            patch_points: Vec::new(),
            patch_params: Vec::new(),

            tree_nodes: Vec::new(),
            tree_depth: -1,

            stencil_matrix_f32: Vec::new(),
            stencil_matrix_f64: Vec::new(),
        }
    }

    // -----------------------------------------------------------------------
    // Simple accessors
    // -----------------------------------------------------------------------

    #[inline]
    pub fn get_num_control_points(&self) -> i32 {
        self.num_control_points
    }
    #[inline]
    pub fn get_num_sub_patch_points(&self) -> i32 {
        self.num_sub_patch_points
    }
    #[inline]
    pub fn get_num_points_total(&self) -> i32 {
        self.num_control_points + self.num_sub_patch_points
    }
    #[inline]
    pub fn get_depth(&self) -> i32 {
        self.tree_depth
    }
    #[inline]
    pub fn get_num_patches(&self) -> i32 {
        self.patch_params.len() as i32
    }

    #[inline]
    pub fn has_sub_faces(&self) -> bool {
        self.num_sub_faces > 0
    }
    #[inline]
    pub fn get_num_sub_faces(&self) -> i32 {
        self.num_sub_faces
    }

    #[inline]
    pub fn uses_double_precision(&self) -> bool {
        self.use_double_precision
    }

    // -----------------------------------------------------------------------
    // Stencil matrix access
    // -----------------------------------------------------------------------

    pub fn get_stencil_matrix_f32(&self) -> &[f32] {
        debug_assert!(!self.stencil_matrix_f32.is_empty());
        &self.stencil_matrix_f32
    }

    pub fn get_stencil_matrix_f64(&self) -> &[f64] {
        debug_assert!(!self.stencil_matrix_f64.is_empty());
        &self.stencil_matrix_f64
    }

    // -----------------------------------------------------------------------
    // Sub-patch access
    // -----------------------------------------------------------------------

    /// Return the point indices used by sub-patch `patch_index`.
    ///
    /// Mirrors `PatchTree::GetSubPatchPoints`.
    pub fn get_sub_patch_points(&self, patch_index: i32) -> &[i32] {
        let pi = patch_index as usize;
        let sz = if self.patch_params[pi].is_regular() {
            self.reg_patch_size
        } else {
            self.irreg_patch_size
        } as usize;
        let off = pi * self.patch_point_stride as usize;
        &self.patch_points[off..off + sz]
    }

    /// Return the `PatchParam` for sub-patch `patch_index`.
    #[inline]
    pub fn get_sub_patch_param(&self, patch_index: i32) -> PatchParam {
        self.patch_params[patch_index as usize]
    }

    // -----------------------------------------------------------------------
    // FindSubPatch / search
    // -----------------------------------------------------------------------

    /// Find the index of the sub-patch containing `(u, v)` within `sub_face`.
    ///
    /// `search_depth`: optional max search depth (-1 = full tree depth).
    /// Mirrors `PatchTree::FindSubPatch` / `searchQuadtree`.
    pub fn find_sub_patch(&self, u: f64, v: f64, sub_face: i32, search_depth: i32) -> i32 {
        self.search_quadtree(u, v, sub_face, search_depth)
    }

    fn search_quadtree(&self, mut u: f64, mut v: f64, sub_face: i32, search_depth: i32) -> i32 {
        let node0 = &self.tree_nodes[sub_face as usize];

        if self.tree_depth == 0 {
            debug_assert!(node0.patch_index >= 0);
            return node0.patch_index;
        }

        let mut max_depth = if search_depth >= 0 && self.patches_include_non_leaf {
            search_depth
        } else {
            self.tree_depth
        };

        // Guard: when max_depth equals the root depth (1 for sub-faces, 0 otherwise),
        // try returning the root patch if one exists; otherwise force depth to 1.
        // Mirrors C++: if (maxDepth == (_numSubFaces > 0)) { ... }
        if max_depth == (self.num_sub_faces > 0) as i32 {
            if node0.patch_index >= 0 {
                return node0.patch_index;
            }
            max_depth = 1;
        }

        let mut node = node0;
        let mut median = 0.5f64;
        let mut tri_rotated = false;

        for _depth in 1..=max_depth {
            let quadrant = if self.patches_are_triangular {
                transform_uv_to_tri_quadrant(median, &mut u, &mut v, &mut tri_rotated)
            } else {
                transform_uv_to_quad_quadrant(median, &mut u, &mut v)
            };

            let child = &node.children[quadrant as usize];
            if child.is_leaf {
                return child.get_index();
            } else if child.is_set {
                node = &self.tree_nodes[child.get_index() as usize];
            }

            median *= 0.5;
        }
        debug_assert!(node.patch_index >= 0);
        node.patch_index
    }

    // -----------------------------------------------------------------------
    // Evaluate sub-patch basis (weights)
    // -----------------------------------------------------------------------

    /// Evaluate basis weights for the given sub-patch.
    ///
    /// Mirrors `PatchTree::EvalSubPatchBasis<REAL>`.
    pub fn eval_sub_patch_basis_f32(
        &self,
        patch_index: i32,
        u: f32,
        v: f32,
        w: Option<&mut [f32]>,
        wdu: Option<&mut [f32]>,
        wdv: Option<&mut [f32]>,
        wduu: Option<&mut [f32]>,
        wduv: Option<&mut [f32]>,
        wdvv: Option<&mut [f32]>,
    ) -> i32 {
        let param = self.patch_params[patch_index as usize];
        let ptype = if param.is_regular() {
            self.reg_patch_type
        } else {
            self.irreg_patch_type
        };

        crate::far::evaluate_patch_basis(ptype, param, u, v, w, wdu, wdv, wduu, wduv, wdvv)
    }

    pub fn eval_sub_patch_basis_f64(
        &self,
        patch_index: i32,
        u: f64,
        v: f64,
        w: Option<&mut [f64]>,
        wdu: Option<&mut [f64]>,
        wdv: Option<&mut [f64]>,
        wduu: Option<&mut [f64]>,
        wduv: Option<&mut [f64]>,
        wdvv: Option<&mut [f64]>,
    ) -> i32 {
        let param = self.patch_params[patch_index as usize];
        let ptype = if param.is_regular() {
            self.reg_patch_type
        } else {
            self.irreg_patch_type
        };

        crate::far::evaluate_patch_basis_f64(ptype, param, u, v, w, wdu, wdv, wduu, wduv, wdvv)
    }

    // -----------------------------------------------------------------------
    // Evaluate stencils (coefficients in terms of control points)
    // -----------------------------------------------------------------------

    /// Evaluate limit stencil weights for sub-patch `patch_index`.
    ///
    /// Returns `num_control_points`.
    /// Mirrors `PatchTree::EvalSubPatchStencils<REAL>`.
    pub fn eval_sub_patch_stencils_f32(
        &self,
        patch_index: i32,
        u: f32,
        v: f32,
        sp: &mut [f32],
        sdu: Option<&mut [f32]>,
        sdv: Option<&mut [f32]>,
        sduu: Option<&mut [f32]>,
        sduv: Option<&mut [f32]>,
        sdvv: Option<&mut [f32]>,
    ) -> i32 {
        let param = self.patch_params[patch_index as usize];

        // Fast path: regular interior base-level patch — weights = stencils.
        if param.get_depth() == 0 && param.is_regular() && param.get_boundary() == 0 {
            debug_assert_eq!(self.reg_patch_size, self.num_control_points);
            return crate::far::evaluate_patch_basis(
                self.reg_patch_type,
                param,
                u,
                v,
                Some(sp),
                sdu,
                sdv,
                sduu,
                sduv,
                sdvv,
            );
        }

        if self.use_double_precision {
            self.eval_stencils_impl::<f64, f32>(
                patch_index,
                u as f64,
                v as f64,
                sp,
                sdu,
                sdv,
                sduu,
                sduv,
                sdvv,
            )
        } else {
            self.eval_stencils_impl::<f32, f32>(
                patch_index,
                u as f64,
                v as f64,
                sp,
                sdu,
                sdv,
                sduu,
                sduv,
                sdvv,
            )
        }
    }

    pub fn eval_sub_patch_stencils_f64(
        &self,
        patch_index: i32,
        u: f64,
        v: f64,
        sp: &mut [f64],
        sdu: Option<&mut [f64]>,
        sdv: Option<&mut [f64]>,
        sduu: Option<&mut [f64]>,
        sduv: Option<&mut [f64]>,
        sdvv: Option<&mut [f64]>,
    ) -> i32 {
        let param = self.patch_params[patch_index as usize];

        if param.get_depth() == 0 && param.is_regular() && param.get_boundary() == 0 {
            debug_assert_eq!(self.reg_patch_size, self.num_control_points);
            return crate::far::evaluate_patch_basis_f64(
                self.reg_patch_type,
                param,
                u,
                v,
                Some(sp),
                sdu,
                sdv,
                sduu,
                sduv,
                sdvv,
            );
        }

        if self.use_double_precision {
            self.eval_stencils_impl::<f64, f64>(patch_index, u, v, sp, sdu, sdv, sduu, sduv, sdvv)
        } else {
            self.eval_stencils_impl::<f32, f64>(patch_index, u, v, sp, sdu, sdv, sduu, sduv, sdvv)
        }
    }

    /// Generic stencil evaluation: `REAL_MATRIX` = stencil storage precision,
    /// `REAL_OUT` = output precision.
    fn eval_stencils_impl<RealMatrix, RealOut>(
        &self,
        patch_index: i32,
        u: f64,
        v: f64,
        sp: &mut [RealOut],
        mut sdu: Option<&mut [RealOut]>,
        mut sdv: Option<&mut [RealOut]>,
        mut sduu: Option<&mut [RealOut]>,
        mut sduv: Option<&mut [RealOut]>,
        mut sdvv: Option<&mut [RealOut]>,
    ) -> i32
    where
        RealMatrix: num_traits::Float + 'static,
        RealOut: num_traits::Float
            + num_traits::AsPrimitive<RealOut>
            + Copy
            + std::ops::AddAssign
            + 'static,
        f64: num_traits::AsPrimitive<RealOut>,
        RealMatrix: num_traits::AsPrimitive<RealOut>,
    {
        let nc = self.num_control_points as usize;

        // Basis weights at patch level.
        let mut wp = vec![0.0f64; 20];
        let mut wdu = vec![0.0f64; 20];
        let mut wdv = vec![0.0f64; 20];
        let mut wduu = vec![0.0f64; 20];
        let mut wduv = vec![0.0f64; 20];
        let mut wdvv = vec![0.0f64; 20];

        let d1 = sdu.is_some() && sdv.is_some();
        let d2 = d1 && sduu.is_some() && sduv.is_some() && sdvv.is_some();

        let param = self.patch_params[patch_index as usize];
        let ptype = if param.is_regular() {
            self.reg_patch_type
        } else {
            self.irreg_patch_type
        };

        crate::far::evaluate_patch_basis_f64(
            ptype,
            param,
            u,
            v,
            Some(&mut wp),
            if d1 { Some(&mut wdu) } else { None },
            if d1 { Some(&mut wdv) } else { None },
            if d2 { Some(&mut wduu) } else { None },
            if d2 { Some(&mut wduv) } else { None },
            if d2 { Some(&mut wdvv) } else { None },
        );

        let patch_pts = self.get_sub_patch_points(patch_index);

        // Zero output stencils.
        for s in sp.iter_mut() {
            *s = RealOut::zero();
        }
        if d1 {
            if let (Some(ref mut du), Some(ref mut dv)) = (sdu.as_deref_mut(), sdv.as_deref_mut()) {
                for s in du.iter_mut() {
                    *s = RealOut::zero();
                }
                for s in dv.iter_mut() {
                    *s = RealOut::zero();
                }
            }
        }
        if d2 {
            if let (Some(ref mut duu), Some(ref mut duv), Some(ref mut dvv)) = (
                sduu.as_deref_mut(),
                sduv.as_deref_mut(),
                sdvv.as_deref_mut(),
            ) {
                for s in duu.iter_mut() {
                    *s = RealOut::zero();
                }
                for s in duv.iter_mut() {
                    *s = RealOut::zero();
                }
                for s in dvv.iter_mut() {
                    *s = RealOut::zero();
                }
            }
        }

        // Accumulate contributions.
        let stencil_row = if std::mem::size_of::<RealMatrix>() == 8 {
            None::<&[f32]>
        } else {
            None::<&[f32]>
        };
        let _ = stencil_row; // will be resolved per-branch below

        for (i, &pi) in patch_pts.iter().enumerate() {
            let pi = pi as usize;
            if pi < nc {
                sp[pi] = sp[pi] + num_traits::cast(wp[i]).unwrap_or(RealOut::zero());
                if d1 {
                    if let (Some(ref mut du), Some(ref mut dv)) =
                        (sdu.as_deref_mut(), sdv.as_deref_mut())
                    {
                        du[pi] = du[pi] + num_traits::cast(wdu[i]).unwrap_or(RealOut::zero());
                        dv[pi] = dv[pi] + num_traits::cast(wdv[i]).unwrap_or(RealOut::zero());
                    }
                }
                if d2 {
                    if let (Some(ref mut duu), Some(ref mut duv), Some(ref mut dvv)) = (
                        sduu.as_deref_mut(),
                        sduv.as_deref_mut(),
                        sdvv.as_deref_mut(),
                    ) {
                        duu[pi] = duu[pi] + num_traits::cast(wduu[i]).unwrap_or(RealOut::zero());
                        duv[pi] = duv[pi] + num_traits::cast(wduv[i]).unwrap_or(RealOut::zero());
                        dvv[pi] = dvv[pi] + num_traits::cast(wdvv[i]).unwrap_or(RealOut::zero());
                    }
                }
            } else {
                // pi refers to a refined point — look up its stencil row.
                let row_off = (pi - nc) * nc;
                let w_p: RealOut = num_traits::cast(wp[i]).unwrap_or(RealOut::zero());

                if self.use_double_precision {
                    let row = &self.stencil_matrix_f64[row_off..row_off + nc];
                    for j in 0..nc {
                        sp[j] = sp[j] + w_p * num_traits::cast(row[j]).unwrap_or(RealOut::zero());
                    }
                    if d1 {
                        let wd_u: RealOut = num_traits::cast(wdu[i]).unwrap_or(RealOut::zero());
                        let wd_v: RealOut = num_traits::cast(wdv[i]).unwrap_or(RealOut::zero());
                        if let (Some(ref mut du), Some(ref mut dv)) =
                            (sdu.as_deref_mut(), sdv.as_deref_mut())
                        {
                            for j in 0..nc {
                                let rv: RealOut =
                                    num_traits::cast(row[j]).unwrap_or(RealOut::zero());
                                du[j] = du[j] + wd_u * rv;
                                dv[j] = dv[j] + wd_v * rv;
                            }
                        }
                    }
                    if d2 {
                        let wd_uu: RealOut = num_traits::cast(wduu[i]).unwrap_or(RealOut::zero());
                        let wd_uv: RealOut = num_traits::cast(wduv[i]).unwrap_or(RealOut::zero());
                        let wd_vv: RealOut = num_traits::cast(wdvv[i]).unwrap_or(RealOut::zero());
                        if let (Some(ref mut duu), Some(ref mut duv), Some(ref mut dvv)) = (
                            sduu.as_deref_mut(),
                            sduv.as_deref_mut(),
                            sdvv.as_deref_mut(),
                        ) {
                            for j in 0..nc {
                                let rv: RealOut =
                                    num_traits::cast(row[j]).unwrap_or(RealOut::zero());
                                duu[j] = duu[j] + wd_uu * rv;
                                duv[j] = duv[j] + wd_uv * rv;
                                dvv[j] = dvv[j] + wd_vv * rv;
                            }
                        }
                    }
                } else {
                    let row = &self.stencil_matrix_f32[row_off..row_off + nc];
                    for j in 0..nc {
                        sp[j] = sp[j] + w_p * num_traits::cast(row[j]).unwrap_or(RealOut::zero());
                    }
                    if d1 {
                        let wd_u: RealOut = num_traits::cast(wdu[i]).unwrap_or(RealOut::zero());
                        let wd_v: RealOut = num_traits::cast(wdv[i]).unwrap_or(RealOut::zero());
                        if let (Some(ref mut du), Some(ref mut dv)) =
                            (sdu.as_deref_mut(), sdv.as_deref_mut())
                        {
                            for j in 0..nc {
                                let rv: RealOut =
                                    num_traits::cast(row[j]).unwrap_or(RealOut::zero());
                                du[j] = du[j] + wd_u * rv;
                                dv[j] = dv[j] + wd_v * rv;
                            }
                        }
                    }
                    if d2 {
                        let wd_uu: RealOut = num_traits::cast(wduu[i]).unwrap_or(RealOut::zero());
                        let wd_uv: RealOut = num_traits::cast(wduv[i]).unwrap_or(RealOut::zero());
                        let wd_vv: RealOut = num_traits::cast(wdvv[i]).unwrap_or(RealOut::zero());
                        if let (Some(ref mut duu), Some(ref mut duv), Some(ref mut dvv)) = (
                            sduu.as_deref_mut(),
                            sduv.as_deref_mut(),
                            sdvv.as_deref_mut(),
                        ) {
                            for j in 0..nc {
                                let rv: RealOut =
                                    num_traits::cast(row[j]).unwrap_or(RealOut::zero());
                                duu[j] = duu[j] + wd_uu * rv;
                                duv[j] = duv[j] + wd_uv * rv;
                                dvv[j] = dvv[j] + wd_vv * rv;
                            }
                        }
                    }
                }
            }
        }
        self.num_control_points
    }

    // -----------------------------------------------------------------------
    // Build quadtree (called by PatchTreeBuilder after assembling patches)
    // -----------------------------------------------------------------------

    /// Build the internal quad-tree from the assembled `patch_params`.
    ///
    /// Mirrors `PatchTree::buildQuadtree`.
    pub(crate) fn build_quadtree(&mut self) {
        let num_patches = self.patch_params.len();

        self.tree_nodes.clear();
        self.tree_nodes.reserve(num_patches);
        let root_count = if self.num_sub_faces > 0 {
            self.num_sub_faces as usize
        } else {
            1
        };
        self.tree_nodes.resize_with(root_count, TreeNode::default);
        self.tree_depth = 0;

        for patch_index in 0..num_patches {
            let param = self.patch_params[patch_index];

            let depth = param.get_depth();
            let root_depth = if param.non_quad_root() { 1 } else { 0 };
            let sub_face = param.get_face_id();

            let _ = self.tree_nodes[sub_face as usize].patch_index; // ensure in range

            if depth > self.tree_depth {
                self.tree_depth = depth;
            }

            if depth == root_depth {
                self.tree_nodes[sub_face as usize].patch_index = patch_index as i32;
                continue;
            }

            if !self.patches_are_triangular {
                let u = param.get_u();
                let v = param.get_v();

                let mut node_idx = sub_face as usize;
                for j in (root_depth + 1)..=depth {
                    let shift = depth - j;
                    let u_bit = (u >> shift) & 1;
                    let v_bit = (v >> shift) & 1;
                    let quadrant = ((v_bit << 1) | u_bit) as usize;
                    let is_leaf = j == depth;

                    node_idx =
                        self.assign_leaf_or_child(node_idx, is_leaf, quadrant, patch_index as i32);
                }
            } else {
                let mut u = 0.25f64;
                let mut v = 0.25f64;
                param.unnormalize_triangle(&mut u, &mut v);

                let mut median = 0.5f64;
                let mut rotated = false;
                let mut node_idx = sub_face as usize;

                for j in (root_depth + 1)..=depth {
                    let quadrant =
                        transform_uv_to_tri_quadrant(median, &mut u, &mut v, &mut rotated) as usize;
                    let is_leaf = j == depth;

                    node_idx =
                        self.assign_leaf_or_child(node_idx, is_leaf, quadrant, patch_index as i32);
                    median *= 0.5;
                }
            }
        }
    }

    /// Returns the resulting node index.
    fn assign_leaf_or_child(
        &mut self,
        node_idx: usize,
        is_leaf: bool,
        quadrant: usize,
        patch_idx: i32,
    ) -> usize {
        if !self.tree_nodes[node_idx].children[quadrant].is_set {
            if is_leaf {
                self.tree_nodes[node_idx].set_child(quadrant, patch_idx, true);
                return node_idx;
            } else {
                let new_idx = self.tree_nodes.len();
                self.tree_nodes.push(TreeNode::default());
                self.tree_nodes[node_idx].set_child(quadrant, new_idx as i32, false);
                return new_idx;
            }
        }

        if is_leaf || self.tree_nodes[node_idx].children[quadrant].is_leaf {
            let new_idx = self.tree_nodes.len();
            self.tree_nodes.push(TreeNode::default());

            let existing_pi = self.tree_nodes[node_idx].children[quadrant].get_index();
            self.tree_nodes[new_idx].patch_index = existing_pi;

            self.tree_nodes[node_idx].children[quadrant].set_index(new_idx as i32);
            self.tree_nodes[node_idx].children[quadrant].is_leaf = false;

            if is_leaf {
                self.tree_nodes[new_idx].set_child(quadrant, patch_idx, true);
            }
            return new_idx;
        }

        self.tree_nodes[node_idx].children[quadrant].get_index() as usize
    }
}

// ---------------------------------------------------------------------------
// UV-space helpers (local, mirroring C++ anonymous-namespace functions)
// ---------------------------------------------------------------------------

/// Map `(u,v)` to a quad quadrant `[0,3]`, adjusting `u` and `v` in place.
#[inline]
fn transform_uv_to_quad_quadrant(median: f64, u: &mut f64, v: &mut f64) -> i32 {
    let u_half = (*u >= median) as i32;
    if u_half != 0 {
        *u -= median;
    }
    let v_half = (*v >= median) as i32;
    if v_half != 0 {
        *v -= median;
    }
    (v_half << 1) | u_half
}

/// Map `(u,v)` to a triangular quadrant `[0,3]`.
#[inline]
fn transform_uv_to_tri_quadrant(median: f64, u: &mut f64, v: &mut f64, rotated: &mut bool) -> i32 {
    if !*rotated {
        if *u >= median {
            *u -= median;
            return 1;
        }
        if *v >= median {
            *v -= median;
            return 2;
        }
        if *u + *v >= median {
            *rotated = true;
            return 3;
        }
        0
    } else {
        if *u < median {
            *v -= median;
            return 1;
        }
        if *v < median {
            *u -= median;
            return 2;
        }
        *u -= median;
        *v -= median;
        if *u + *v < median {
            *rotated = true;
            return 3;
        }
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quad_quadrant_basic() {
        let mut u = 0.75f64;
        let mut v = 0.25f64;
        let q = transform_uv_to_quad_quadrant(0.5, &mut u, &mut v);
        // u >= 0.5 -> u_half = 1, u becomes 0.25
        // v < 0.5  -> v_half = 0
        assert_eq!(q, 1);
        assert!((u - 0.25).abs() < 1e-12);
    }

    #[test]
    fn tree_node_default_has_no_children() {
        let n = TreeNode::default();
        assert_eq!(n.patch_index, -1);
        assert!(!n.children[0].is_set);
    }

    #[test]
    fn tree_node_set_children_marks_all_leaves() {
        let mut n = TreeNode::default();
        n.set_children(42);
        for c in &n.children {
            assert!(c.is_set);
            assert!(c.is_leaf);
            assert_eq!(c.get_index(), 42);
        }
    }
}
