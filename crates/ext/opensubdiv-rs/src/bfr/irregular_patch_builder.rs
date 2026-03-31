//! IrregularPatchBuilder — builds the IrregularPatchType for a non-regular face.
//!
//! Ported from OpenSubdiv bfr/irregularPatchBuilder.h/.cpp.
//!
//! This class assembles the "control hull" (set of control vertices and
//! incident faces) from a `FaceSurface` and then constructs a `PatchTree`
//! via `PatchTreeBuilder`.

use std::sync::Arc;
use std::collections::HashMap;

use crate::sdc::crease::SHARPNESS_INFINITE;
use super::face_surface::FaceSurface;
use super::irregular_patch_type::IrregularPatchSharedPtr;

pub type Index = super::face_surface::Index;

// ---------------------------------------------------------------------------
//  Options
// ---------------------------------------------------------------------------

/// Construction options for `IrregularPatchBuilder`.
#[derive(Clone, Copy, Debug)]
pub struct IrregPatchOptions {
    /// Maximum refinement depth for sharp features.
    pub sharp_level: u8,
    /// Maximum refinement depth for smooth features.
    pub smooth_level: u8,
    /// Use double-precision stencil matrix.
    pub double_precision: bool,
}

impl Default for IrregPatchOptions {
    fn default() -> Self {
        IrregPatchOptions {
            sharp_level:      6,
            smooth_level:     2,
            double_precision: false,
        }
    }
}

// ---------------------------------------------------------------------------
//  CornerHull — per-corner contribution inventory
// ---------------------------------------------------------------------------

/// Records how many control vertices/faces a given corner contributes to the
/// collective control hull.
#[derive(Clone, Default, Debug)]
struct CornerHull {
    num_control_faces:     i32,
    num_control_verts:     i32,
    next_control_vert:     i32,
    surface_indices_offset: i32,
    is_val2_interior:      bool,
    pre_val2_interior:     bool,
    single_shared_vert:    bool,
    single_shared_face:    bool,
}

// ---------------------------------------------------------------------------
//  IrregularPatchBuilder
// ---------------------------------------------------------------------------

/// Assembles the control hull and builds a `PatchTree` for one irregular face.
///
/// Mirrors `Bfr::IrregularPatchBuilder`.
pub struct IrregularPatchBuilder<'a> {
    surface:                &'a FaceSurface<'a>,
    options:                IrregPatchOptions,

    num_control_verts:      i32,
    num_control_faces:      i32,
    num_control_face_verts: i32,
    control_faces_overlap:  bool,
    use_control_vert_map:   bool,

    corner_hulls:           Vec<CornerHull>,

    // Only populated when use_control_vert_map == true:
    control_vert_map:       HashMap<Index, i32>,
    control_verts:          Vec<Index>,
}

impl<'a> IrregularPatchBuilder<'a> {
    /// Construct the builder; performs inventory of the control hull.
    pub fn new(surface: &'a FaceSurface<'a>, options: IrregPatchOptions) -> Self {
        let face_size = surface.get_face_size() as usize;
        let mut builder = IrregularPatchBuilder {
            surface,
            options,
            num_control_verts:      0,
            num_control_faces:      0,
            num_control_face_verts: 0,
            control_faces_overlap:  false,
            use_control_vert_map:   false,
            corner_hulls:           vec![CornerHull::default(); face_size],
            control_vert_map:       HashMap::new(),
            control_verts:          Vec::new(),
        };
        builder.initialize_control_hull_inventory();
        builder
    }

    pub fn with_default_options(surface: &'a FaceSurface<'a>) -> Self {
        Self::new(surface, IrregPatchOptions::default())
    }

    // -----------------------------------------------------------------------
    //  Public queries
    // -----------------------------------------------------------------------

    pub fn get_num_control_vertices(&self) -> i32 { self.num_control_verts }
    pub fn control_hull_depends_on_mesh_indices(&self) -> bool { self.use_control_vert_map }

    /// Fill `cv_indices` with the mesh indices of control vertices.
    pub fn gather_control_vertex_indices(&self, cv_indices: &mut [Index]) -> i32 {
        if self.use_control_vert_map {
            let n = self.control_verts.len();
            cv_indices[..n].copy_from_slice(&self.control_verts);
            return n as i32;
        }

        let face_size = self.surface.get_face_size() as usize;
        let base_face_indices = self.get_base_face_indices();

        let mut num = face_size;
        cv_indices[..face_size].copy_from_slice(base_face_indices);

        for corner in 0..face_size {
            let hull = &self.corner_hulls[corner];
            if hull.num_control_verts == 0 { continue; }

            let c_top = self.surface.get_corner_topology(corner);
            let c_sub = self.surface.get_corner_subset(corner);

            if hull.single_shared_vert {
                let fi = c_top.get_face_after(2);
                let idx = self.get_corner_indices(corner);
                let fv = c_top.get_face_index_offset(fi) as usize;
                cv_indices[num] = idx[fv + 1];
                num += 1;
                continue;
            }

            let idx = self.get_corner_indices(corner);

            // Faces after the corner face:
            if c_sub.num_faces_after > 1 {
                let mut next_face = c_top.get_face_after(1);
                let n_after = c_sub.num_faces_after as usize - 1;
                for j in 0..n_after {
                    next_face = c_top.get_face_next(next_face);
                    let fv_off = c_top.get_face_index_offset(next_face) as usize;
                    let s = c_top.get_face_size(next_face) as usize;
                    let l = if j == n_after - 1 { 1 + hull.pre_val2_interior as usize } else { 0 };
                    let m = (s - 2) - if c_sub.is_boundary() { 0 } else { l };
                    for k in 1..=m {
                        cv_indices[num] = idx[fv_off + k];
                        num += 1;
                    }
                }
            }
            if c_sub.num_faces_after > 0 && c_sub.is_boundary() {
                // Include trailing edge vertex for boundary.
                let fi_trail = c_top.get_face_after(c_sub.num_faces_after as i32);
                cv_indices[num] = c_top.get_face_index_trailing(fi_trail, idx);
                num += 1;
            }
            if c_sub.num_faces_before > 0 {
                let mut next_face = c_top.get_face_first(c_sub);
                let n_before = c_sub.num_faces_before as usize;
                for j in 0..n_before {
                    let fv_off = c_top.get_face_index_offset(next_face) as usize;
                    let s = c_top.get_face_size(next_face) as usize;
                    let l = if j == n_before - 1 { 1 + hull.pre_val2_interior as usize } else { 0 };
                    let m = (s - 2) - l;
                    for k in 1..=m {
                        cv_indices[num] = idx[fv_off + k];
                        num += 1;
                    }
                    next_face = c_top.get_face_next(next_face);
                }
            }
        }
        debug_assert_eq!(num as i32, self.num_control_verts);
        num as i32
    }

    // -----------------------------------------------------------------------
    //  Build the PatchTree
    // -----------------------------------------------------------------------

    /// Build and return the `IrregularPatchType` (a `PatchTree`) for this face.
    ///
    /// Assembles the full Far::TopologyDescriptor from the control hull, creates
    /// a TopologyRefiner, runs adaptive refinement, and builds a PatchTree via
    /// PatchTreeBuilder — full C++ parity.
    pub fn build(&self) -> IrregularPatchSharedPtr {
        use crate::far::{
            TopologyDescriptor,
            TopologyDescriptorFactory, TopologyRefinerFactory,
            topology_refiner_factory::FactoryOptions,
        };
        use super::patch_tree_builder::{PatchTreeBuilderOptions, RefinerFaceAdapter, IrregularBasis};

        let face_size = self.surface.get_face_size() as i32;

        // Step 1: Allocate topology arrays sized by worst-case bounds.
        let num_faces     = self.num_control_faces;
        let num_face_verts = self.num_control_face_verts;
        let max_corners   = face_size;          // one per base face corner
        let max_creases   = self.num_control_verts; // one per hull edge at most

        let mut face_sizes        = vec![0i32; num_faces as usize];
        let mut face_vert_indices = vec![0i32; num_face_verts as usize];
        let mut corner_indices    = vec![0i32; max_corners as usize];
        let mut crease_indices    = vec![0i32; (max_creases * 2) as usize];
        let mut corner_weights    = vec![0.0f32; max_corners as usize];
        let mut crease_weights    = vec![0.0f32; max_creases as usize];

        // Step 2: Gather local face topology.
        self.gather_control_faces(&mut face_sizes, &mut face_vert_indices);

        // Step 3: Gather sharpness data (only when the surface has sharp features).
        let tag = self.surface.get_tag();

        let num_corners = if tag.has_sharp_vertices() {
            self.gather_control_vertex_sharpness(&mut corner_indices, &mut corner_weights)
        } else {
            0
        };

        let mut num_creases = if tag.has_sharp_edges() {
            self.gather_control_edge_sharpness(&mut crease_indices, &mut crease_weights)
        } else {
            0
        };

        // Step 4: Overlap adjustments — deduplicate faces and sharpen boundary edges.
        let mut actual_num_faces  = num_faces;
        let mut actual_num_fverts = num_face_verts;
        if self.control_faces_overlap {
            if actual_num_faces > 2 {
                Self::remove_duplicate_control_faces(
                    &mut face_sizes,
                    &mut face_vert_indices,
                    &mut actual_num_faces,
                    &mut actual_num_fverts,
                );
            }
            if tag.has_boundary_vertices() {
                self.sharpen_boundary_control_edges(
                    &mut crease_indices,
                    &mut crease_weights,
                    &mut num_creases,
                );
            }
        }

        // Step 5: Build Far::TopologyDescriptor from the gathered arrays.
        let mut desc = TopologyDescriptor {
            num_vertices:          self.num_control_verts,
            num_faces:             actual_num_faces,
            num_verts_per_face:    face_sizes[..actual_num_faces as usize].to_vec(),
            vert_indices_per_face: face_vert_indices[..actual_num_fverts as usize].to_vec(),
            ..TopologyDescriptor::default()
        };
        if num_corners > 0 {
            desc.num_corners             = num_corners;
            desc.corner_vertex_indices   = corner_indices[..num_corners as usize].to_vec();
            desc.corner_weights          = corner_weights[..num_corners as usize].to_vec();
        }
        if num_creases > 0 {
            desc.num_creases                = num_creases;
            desc.crease_vertex_index_pairs  = crease_indices[..(num_creases * 2) as usize].to_vec();
            desc.crease_weights             = crease_weights[..num_creases as usize].to_vec();
        }

        // Step 6: Create Far::TopologyRefiner for the local control hull.
        let scheme_type    = self.surface.get_sdc_scheme();
        let scheme_options = self.surface.get_sdc_options_in_effect();
        let factory_opts   = FactoryOptions::new(scheme_type, scheme_options);

        let mut refiner = TopologyDescriptorFactory::create(&desc, factory_opts)
            .expect("IrregularPatchBuilder: failed to create TopologyRefiner");

        // Step 7: Apply adaptive refinement and build PatchTree via PatchTreeBuilder.
        let ptb_opts = PatchTreeBuilderOptions {
            irregular_basis:          IrregularBasis::Gregory,
            max_patch_depth_sharp:    self.options.sharp_level,
            max_patch_depth_smooth:   self.options.smooth_level,
            use_double_precision:     self.options.double_precision,
            include_interior_patches: false,
        };

        let adapter = RefinerFaceAdapter::refine_and_create(&mut refiner, &ptb_opts);
        let tree    = adapter.build_patch_tree(&ptb_opts);

        debug_assert_eq!(tree.num_control_points, self.num_control_verts,
            "PatchTree CV count {} != hull CV count {}",
            tree.num_control_points, self.num_control_verts);

        Arc::new(*tree)
    }

    // -----------------------------------------------------------------------
    //  Private: gather local face topology for TopologyDescriptor
    // -----------------------------------------------------------------------

    /// Assembles `face_sizes[]` and `face_verts[]` using local CV indices (0..N-1).
    ///
    /// Base face is always first: `[0, 1, ..., face_size-1]`.
    /// Per-corner faces follow, using the same traversal pattern as
    /// `gather_control_vertex_indices` but emitting local indices.
    fn gather_control_faces(&self, face_sizes: &mut [i32], face_verts: &mut [i32]) {
        let face_size  = self.surface.get_face_size() as usize;
        let num_cverts = self.num_control_verts;

        // --- Base face: [0, 1, ..., face_size-1] ---
        face_sizes[0] = face_size as i32;
        for i in 0..face_size {
            face_verts[i] = i as i32;
        }

        let mut face_out  = 1usize;     // next slot in face_sizes[]
        let mut fvert_out = face_size;  // next slot in face_verts[]

        for corner in 0..face_size {
            let hull  = &self.corner_hulls[corner];
            if hull.num_control_faces == 0 { continue; }

            let c_top  = self.surface.get_corner_topology(corner);
            let c_sub  = self.surface.get_corner_subset(corner);
            let src_idx = self.get_corner_indices(corner);

            // ---- singleSharedFace: one back-to-back face (all-val2 case) ----
            if hull.single_shared_face {
                let nf   = c_top.get_face_after(1);
                let s    = c_top.get_face_size(nf) as usize;
                let foff = c_top.get_face_index_offset(nf) as usize;
                face_sizes[face_out] = s as i32;
                self.get_control_face_vertices_map(
                    &mut face_verts[fvert_out..fvert_out + s],
                    s, corner as i32, &src_idx[foff..]);
                face_out  += 1;
                fvert_out += s;
                continue;
            }

            // Track the next sequential perimeter vertex index.
            let mut next_vert = hull.next_control_vert;

            // ---- Faces AFTER the corner face ----
            if c_sub.num_faces_after > 1 {
                let mut next_face = c_top.get_face_after(1);
                let n_after = c_sub.num_faces_after as usize - 1;
                for j in 0..n_after {
                    next_face = c_top.get_face_next(next_face);
                    let s    = c_top.get_face_size(next_face) as usize;
                    let foff = c_top.get_face_index_offset(next_face) as usize;
                    let is_last = j == n_after - 1;

                    face_sizes[face_out] = s as i32;
                    if self.use_control_vert_map {
                        self.get_control_face_vertices_map(
                            &mut face_verts[fvert_out..fvert_out + s],
                            s, corner as i32, &src_idx[foff..]);
                    } else if c_sub.is_boundary() {
                        // Boundary: trivial sequential — no wrap-around needed.
                        self.get_control_face_vertices_seq(
                            &mut face_verts[fvert_out..fvert_out + s],
                            s, corner as i32, next_vert);
                    } else {
                        // Interior general case.
                        let pre_val2 = if is_last { hull.pre_val2_interior as i32 } else { 0 };
                        self.get_control_face_vertices_general(
                            &mut face_verts[fvert_out..fvert_out + s],
                            s, corner as i32, next_vert, is_last, pre_val2, num_cverts, face_size);
                    }

                    // Advance next_vert by the number of new perimeter verts this face adds.
                    // C++ always advances, including for the last face. For the last interior
                    // face the pre_val2_interior flag reduces the step by 1.
                    let advance = if c_sub.is_boundary() {
                        s as i32 - 2
                    } else {
                        let pre = if is_last { hull.pre_val2_interior as i32 } else { 0 };
                        (s as i32 - 2) - pre
                    };
                    next_vert += advance;

                    face_out  += 1;
                    fvert_out += s;
                }
            }

            // Boundary trailing-edge gap: one boundary-edge vertex was added.
            if c_sub.num_faces_after > 0 && c_sub.is_boundary() {
                next_vert += 1;
            }

            // ---- Faces BEFORE the corner face ----
            if c_sub.num_faces_before > 0 {
                let mut next_face = c_top.get_face_first(c_sub);
                let n_before = c_sub.num_faces_before as usize;
                for j in 0..n_before {
                    let s    = c_top.get_face_size(next_face) as usize;
                    let foff = c_top.get_face_index_offset(next_face) as usize;
                    let is_last = j == n_before - 1;

                    face_sizes[face_out] = s as i32;
                    if self.use_control_vert_map {
                        self.get_control_face_vertices_map(
                            &mut face_verts[fvert_out..fvert_out + s],
                            s, corner as i32, &src_idx[foff..]);
                    } else {
                        let pre_val2 = if is_last { hull.pre_val2_interior as i32 } else { 0 };
                        self.get_control_face_vertices_general(
                            &mut face_verts[fvert_out..fvert_out + s],
                            s, corner as i32, next_vert, is_last, pre_val2, num_cverts, face_size);
                    }

                    // Always advance, including for the last face — C++ line 517.
                    let pre = if is_last { hull.pre_val2_interior as i32 } else { 0 };
                    next_vert += (s as i32 - 2) - pre;

                    next_face  = c_top.get_face_next(next_face);
                    face_out  += 1;
                    fvert_out += s;
                }
            }
        }
    }

    // -----------------------------------------------------------------------
    //  Three overloads of getControlFaceVertices
    // -----------------------------------------------------------------------

    /// Map-based variant: looks up every non-corner vertex via `control_vert_map`.
    /// `src_verts` is the start of the face's index block (face_index_offset applied by caller).
    fn get_control_face_vertices_map(
        &self,
        f_verts: &mut [i32],
        _num_f_verts: usize,
        corner: i32,
        src_verts: &[Index],
    ) {
        debug_assert!(self.use_control_vert_map);
        f_verts[0] = corner;
        for (i, v) in f_verts[1..].iter_mut().enumerate() {
            *v = self.get_local_control_vertex(src_verts[i + 1]);
        }
    }

    /// Trivial sequential variant: non-corner verts are `next_perim, next_perim+1, ...`.
    fn get_control_face_vertices_seq(
        &self,
        f_verts: &mut [i32],
        num_f_verts: usize,
        corner: i32,
        next_perim: i32,
    ) {
        f_verts[0] = corner;
        for i in 1..num_f_verts {
            f_verts[i] = next_perim + (i as i32 - 1);
        }
    }

    /// General variant with wrap-around and val-2 adjacent-corner closing.
    ///
    /// After the corner vertex the face outputs:
    /// 1. A simple sequential run of `num_cverts` perimeter verts.
    /// 2. The next-to-last vert, which wraps to `face_size` when it would equal `num_cverts`.
    /// 3. The last vert:
    ///    - If NOT the last face: another perimeter vert (also wrap-checked).
    ///    - If IS the last face: the adjacent corner `(corner+1) % face_size`, plus
    ///      any val-2 interior intermediate corners stepping backward.
    fn get_control_face_vertices_general(
        &self,
        f_verts: &mut [i32],
        num_f_verts: usize,
        corner: i32,
        next_perim: i32,
        last_face: bool,
        num_val2_in_last: i32,
        num_cverts: i32,
        face_size: usize,
    ) {
        // Corner vertex is always first.
        f_verts[0] = corner;

        // Number of "simple" sequential verts before the special last two.
        // Simple count = (S - 2) - 1 - (lastFace ? numVal2InLast : 0)
        let simple_count = (num_f_verts as i32 - 2) - 1
            - if last_face { num_val2_in_last } else { 0 };
        let simple_count = simple_count.max(0) as usize;

        let mut out_idx = 1usize;
        let mut cur_perim = next_perim;

        // 1. Simple sequential run.
        for _ in 0..simple_count {
            f_verts[out_idx] = cur_perim;
            cur_perim += 1;
            out_idx   += 1;
        }

        // 2. Next-to-last perimeter vertex — may wrap to face_size.
        let next_to_last = cur_perim;
        let next_to_last_v = if next_to_last >= num_cverts {
            face_size as i32   // wraps to first non-base-face vert
        } else {
            next_to_last
        };
        cur_perim += 1;

        if num_f_verts > 2 {
            // Only emit next-to-last if the face has room (size >= 3).
            // For a size-3 face, out_idx==1 here; for size-4 it's at 2.
            if out_idx < num_f_verts - 1 {
                f_verts[out_idx] = next_to_last_v;
                out_idx += 1;
            }
        }

        // 3. Last vertex.
        if !last_face {
            // Not the last face: another perimeter vert, also wrap-checked.
            let last_v = if cur_perim >= num_cverts {
                face_size as i32
            } else {
                cur_perim
            };
            f_verts[out_idx] = last_v;
        } else {
            // Last face: close back to the adjacent corner of the base face.
            // C++ order (lines 734-737):
            //   first: intermediate val-2 corners descending (corner+1+i) % N for i in numVal2InLast..1
            //   last:  adjacent corner (corner+1) % N
            for k in (1..=num_val2_in_last as usize).rev() {
                if out_idx < num_f_verts {
                    f_verts[out_idx] = ((corner as usize + 1 + k) % face_size) as i32;
                    out_idx += 1;
                }
            }
            // Adjacent corner is always the final vertex of the last face.
            if out_idx < num_f_verts {
                f_verts[out_idx] = ((corner + 1) as usize % face_size) as i32;
            }
        }
    }

    // -----------------------------------------------------------------------
    //  Private: gather vertex sharpness
    // -----------------------------------------------------------------------

    /// Fills `indices[]` and `weights[]` with sharp base-face corner data.
    /// Returns the number of sharp vertices found.
    ///
    /// Mirrors C++ `gatherControlVertexSharpness()`.
    fn gather_control_vertex_sharpness(
        &self,
        indices: &mut [i32],
        weights: &mut [f32],
    ) -> i32 {
        let face_size = self.surface.get_face_size() as usize;
        let mut count = 0i32;

        for i in 0..face_size {
            let c_sub = self.surface.get_corner_subset(i);

            // is_sharp() == inf_sharp_verts bit on the subset tag.
            if c_sub.is_sharp() {
                indices[count as usize] = i as i32;
                weights[count as usize] = SHARPNESS_INFINITE;
                count += 1;
            } else if c_sub.local_sharpness > 0.0 {
                // Semi-sharp: use the local sharpness stored on the subset.
                indices[count as usize] = i as i32;
                weights[count as usize] = c_sub.local_sharpness;
                count += 1;
            } else {
                // Fall back to raw vertex sharpness from the topology.
                let c_top = self.surface.get_corner_topology(i);
                let vs = c_top.get_vertex_sharpness();
                if vs > 0.0 {
                    indices[count as usize] = i as i32;
                    weights[count as usize] = vs;
                    count += 1;
                }
            }
        }
        count
    }

    // -----------------------------------------------------------------------
    //  Private: gather edge sharpness
    // -----------------------------------------------------------------------

    /// Fills `indices[]` and `weights[]` with sharp edge pairs.
    /// Returns the number of sharp edges found.
    ///
    /// Two passes:
    ///   Pass 1 — base face edges (forward edge of each corner).
    ///   Pass 2 — interior edges from each corner to its perimeter vertices.
    ///
    /// Mirrors C++ `gatherControlEdgeSharpness()`.
    fn gather_control_edge_sharpness(
        &self,
        indices: &mut [i32],
        weights: &mut [f32],
    ) -> i32 {
        let face_size = self.surface.get_face_size() as usize;
        let mut count = 0i32;

        // --- Pass 1: base face edges ---
        // Guard: skip leading-boundary corners (is_boundary && num_faces_before == 0).
        // Those edges are added later by sharpen_boundary_control_edges() to avoid
        // duplicates. C++ line 563: if (!cSub.IsBoundary() || cSub._numFacesBefore).
        for c in 0..face_size {
            let c_top = self.surface.get_corner_topology(c);
            let c_sub = self.surface.get_corner_subset(c);

            if !c_top.get_tag().has_sharp_edges() { continue; }

            // Skip leading-boundary corner: boundary AND no faces before it.
            if c_sub.is_boundary() && c_sub.num_faces_before == 0 { continue; }

            // The "forward" edge of corner c in the base face connects c -> (c+1)%N.
            let corner_face = c_top.get_face();
            let s = c_top.get_face_edge_sharpness(corner_face, false);
            if s > 0.0 {
                let v0 = c as i32;
                let v1 = ((c + 1) % face_size) as i32;
                indices[(count * 2) as usize]     = v0;
                indices[(count * 2 + 1) as usize] = v1;
                weights[count as usize]            = s;
                count += 1;
            }
        }

        // --- Pass 2: interior edges (corner → perimeter vertices) ---
        for c in 0..face_size {
            let hull  = &self.corner_hulls[c];
            if hull.num_control_faces == 0 { continue; }

            let c_top = self.surface.get_corner_topology(c);
            let c_sub = self.surface.get_corner_subset(c);

            if !c_top.get_tag().has_sharp_edges() { continue; }

            let corner_v = c as i32;
            let mut next_vert = hull.next_control_vert;

            // Faces after:
            if c_sub.num_faces_after > 1 {
                let mut next_face = c_top.get_face_after(1);
                let n_after = c_sub.num_faces_after as usize - 1;
                for j in 0..n_after {
                    next_face = c_top.get_face_next(next_face);
                    let s_face = c_top.get_face_size(next_face) as i32;

                    // Trailing edge of this face (the edge between corner and first perim vert).
                    // In C++: GetFaceEdgeSharpness(face, 1) = trailing = true.
                    let edge_s = c_top.get_face_edge_sharpness(next_face, true);
                    if edge_s > 0.0 {
                        let edge_vert = if self.use_control_vert_map {
                            let idx = self.get_corner_indices(c);
                            let foff = c_top.get_face_index_offset(next_face) as usize;
                            self.get_local_control_vertex(idx[foff + 1])
                        } else if next_vert < self.num_control_verts {
                            next_vert
                        } else {
                            face_size as i32  // wrap to first perimeter vert
                        };
                        indices[(count * 2) as usize]     = corner_v;
                        indices[(count * 2 + 1) as usize] = edge_vert;
                        weights[count as usize]            = edge_s;
                        count += 1;
                    }

                    let is_last = j == n_after - 1;
                    let pre = if is_last && !c_sub.is_boundary() {
                        hull.pre_val2_interior as i32
                    } else {
                        0
                    };
                    next_vert += (s_face - 2) - pre;
                }
            }

            // Boundary gap.
            if c_sub.num_faces_after > 0 && c_sub.is_boundary() {
                next_vert += 1;
            }

            // Faces before:
            // C++ line 627: loop starts at i=1 (skips the first face), and advances
            // nextVert BEFORE testing sharpness (advance then test).
            if c_sub.num_faces_before > 0 {
                let mut next_face = c_top.get_face_first(c_sub);
                let n_before = c_sub.num_faces_before as usize;
                // j=0 (first face-before) is skipped — its leading edge is the base-face
                // boundary edge already handled elsewhere. Start at j=1.
                for _j in 1..n_before {
                    // C++ order: advance nextVert, test sharpness of CURRENT
                    // nextFace, then advance nextFace at the end of the loop.
                    let s_prev = c_top.get_face_size(next_face) as i32;
                    next_vert += s_prev - 2;

                    let edge_s = c_top.get_face_edge_sharpness(next_face, true);
                    if edge_s > 0.0 {
                        let edge_vert = if self.use_control_vert_map {
                            let idx = self.get_corner_indices(c);
                            let foff = c_top.get_face_index_offset(next_face) as usize;
                            self.get_local_control_vertex(idx[foff + 1])
                        } else if next_vert < self.num_control_verts {
                            next_vert
                        } else {
                            face_size as i32
                        };
                        indices[(count * 2) as usize]     = corner_v;
                        indices[(count * 2 + 1) as usize] = edge_vert;
                        weights[count as usize]            = edge_s;
                        count += 1;
                    }
                    next_face = c_top.get_face_next(next_face);
                }
            }
        }

        count
    }

    // -----------------------------------------------------------------------
    //  Private: remove duplicate control faces
    // -----------------------------------------------------------------------

    /// Scans backward through the face list (starting at the last face) and
    /// removes any face that is a cyclic rotation of an earlier face.
    ///
    /// Only called when `control_faces_overlap` is true and `num_faces > 2`.
    /// Mirrors C++ `removeDuplicateControlFaces()`.
    fn remove_duplicate_control_faces(
        face_sizes:    &mut Vec<i32>,
        face_verts:    &mut Vec<i32>,
        num_faces:     &mut i32,
        num_face_verts: &mut i32,
    ) {
        // Build per-face vertex-index start offsets.
        let build_offsets = |fs: &[i32], nf: i32| -> Vec<usize> {
            let mut offsets = vec![0usize; nf as usize + 1];
            for i in 0..nf as usize {
                offsets[i + 1] = offsets[i] + fs[i] as usize;
            }
            offsets
        };

        let mut i = *num_faces - 1;
        while i >= 2 {
            let offsets = build_offsets(face_sizes, *num_faces);
            let si  = face_sizes[i as usize] as usize;
            let ai  = &face_verts[offsets[i as usize]..offsets[i as usize] + si];

            let mut found = false;
            for j in 1..i {
                let sj = face_sizes[j as usize] as usize;
                if sj != si { continue; }
                let aj = &face_verts[offsets[j as usize]..offsets[j as usize] + sj];
                if Self::faces_match(ai, aj) {
                    found = true;
                    break;
                }
            }

            if found {
                // Remove face i: shift everything after it down.
                let start = offsets[i as usize];
                let end   = offsets[i as usize + 1];
                let fv_len = *num_face_verts as usize;
                face_verts.copy_within(end..fv_len, start);
                *num_face_verts -= si as i32;
                face_verts.truncate(*num_face_verts as usize);

                face_sizes.copy_within((i as usize + 1)..*num_faces as usize, i as usize);
                *num_faces -= 1;
                face_sizes.truncate(*num_faces as usize);
            } else {
                i -= 1;
            }
        }
    }

    /// Check whether two face vertex arrays are cyclic rotations of each other.
    fn faces_match(a: &[i32], b: &[i32]) -> bool {
        debug_assert_eq!(a.len(), b.len());
        let n = a.len();
        // Find any position in b where b[rot] == a[0].
        for rot in 0..n {
            if b[rot] == a[0] {
                let matches = (0..n).all(|k| b[(rot + k) % n] == a[k]);
                if matches { return true; }
            }
        }
        false
    }

    // -----------------------------------------------------------------------
    //  Private: sharpen leading boundary edges
    // -----------------------------------------------------------------------

    /// For each corner that is a "leading boundary" corner (boundary and no
    /// faces before), appends an infinite-sharpness crease for the edge
    /// `(corner, (corner+1) % face_size)`.
    ///
    /// Mirrors C++ `sharpenBoundaryControlEdges()`.
    fn sharpen_boundary_control_edges(
        &self,
        indices:    &mut [i32],
        weights:    &mut [f32],
        num_creases: &mut i32,
    ) {
        let face_size = self.surface.get_face_size() as usize;

        for c in 0..face_size {
            let c_sub = self.surface.get_corner_subset(c);
            // Leading boundary corner: is_boundary() && num_faces_before == 0.
            if c_sub.is_boundary() && c_sub.num_faces_before == 0 {
                let v0 = c as i32;
                let v1 = ((c + 1) % face_size) as i32;
                let nc = *num_creases as usize;
                indices[nc * 2]     = v0;
                indices[nc * 2 + 1] = v1;
                weights[nc]         = SHARPNESS_INFINITE;
                *num_creases += 1;
            }
        }
    }

    // -----------------------------------------------------------------------
    //  Private: initialize inventory
    // -----------------------------------------------------------------------

    fn initialize_control_hull_inventory(&mut self) {
        let face_size = self.surface.get_face_size() as usize;
        self.corner_hulls.resize(face_size, CornerHull::default());

        let mut num_val2_int_corners = 0i32;
        let mut num_val3_int_adj_tris = 0i32;
        let mut num_src_face_indices = 0i32;

        self.num_control_faces     = 1;
        self.num_control_verts     = face_size as i32;
        self.num_control_face_verts = face_size as i32;

        for corner in 0..face_size {
            let c_top = self.surface.get_corner_topology(corner);
            let c_sub = self.surface.get_corner_subset(corner);
            let hull  = &mut self.corner_hulls[corner];
            *hull = CornerHull::default();

            let mut num_corner_fv = 0i32;

            if c_sub.num_faces_after > 0 {
                let mut next_face = c_top.get_face_next(c_top.get_face());

                if c_sub.is_boundary() {
                    for _ in 1..c_sub.num_faces_after {
                        next_face = c_top.get_face_next(next_face);
                        let s = c_top.get_face_size(next_face);
                        hull.num_control_verts += s - 2;
                        num_corner_fv += s;
                    }
                    hull.num_control_faces = c_sub.num_faces_after as i32 - 1;
                    hull.num_control_verts += 1;
                } else if c_sub.num_faces_total == 3
                    && c_top.get_face_size(c_top.get_face_after(2)) == 3
                {
                    num_val3_int_adj_tris += 1;
                    if num_val3_int_adj_tris == face_size as i32 {
                        hull.single_shared_vert = true;
                        hull.num_control_verts = 1;
                    }
                    hull.num_control_faces = 1;
                    num_corner_fv = 3;
                } else if c_sub.num_faces_total > 2 {
                    for _ in 2..c_sub.num_faces_total {
                        next_face = c_top.get_face_next(next_face);
                        let s = c_top.get_face_size(next_face);
                        hull.num_control_verts += s - 2;
                        num_corner_fv += s;
                    }
                    hull.num_control_faces = c_sub.num_faces_total as i32 - 2;
                    hull.num_control_verts -= 1;
                } else {
                    num_val2_int_corners += 1;
                    if num_val2_int_corners == face_size as i32 {
                        hull.single_shared_face = true;
                        hull.num_control_faces = 1;
                        num_corner_fv = face_size as i32;
                    }
                    hull.is_val2_interior = true;
                }
            }

            if c_sub.num_faces_before > 0 {
                let mut next_face = c_top.get_face_first(c_sub);
                for _ in 0..c_sub.num_faces_before {
                    let s = c_top.get_face_size(next_face);
                    next_face = c_top.get_face_next(next_face);
                    hull.num_control_verts += s - 2;
                    num_corner_fv += s;
                }
                hull.num_control_faces += c_sub.num_faces_before as i32;
                hull.num_control_verts -= 1;
            }

            hull.next_control_vert       = self.num_control_verts;
            hull.surface_indices_offset  = num_src_face_indices;

            self.num_control_faces     += hull.num_control_faces;
            self.num_control_verts     += hull.num_control_verts;
            self.num_control_face_verts += num_corner_fv;

            num_src_face_indices += c_top.get_num_face_vertices();
        }

        // Decide whether to use a vertex map (overlapping faces).
        self.control_faces_overlap = num_val2_int_corners > 1;

        if num_val2_int_corners == 1 {
            for corner in 0..face_size {
                let hull = &self.corner_hulls[corner];
                if hull.is_val2_interior {
                    let c_top = self.surface.get_corner_topology(corner);
                    let opp_size = c_top.get_face_size(c_top.get_face_after(1));
                    if opp_size == 3 {
                        self.control_faces_overlap = true;
                        break;
                    }
                    if opp_size == 4 && num_val3_int_adj_tris == (face_size as i32 - 2) {
                        self.control_faces_overlap = true;
                        break;
                    }
                    // Tag the preceding corner:
                    let prev = if corner == 0 { face_size - 1 } else { corner - 1 };
                    self.corner_hulls[prev].pre_val2_interior = true;
                    break;
                }
            }

            if !self.control_faces_overlap {
                self.num_control_verts = face_size as i32;
                for corner in 0..face_size {
                    let hull = &mut self.corner_hulls[corner];
                    hull.next_control_vert  = self.num_control_verts;
                    hull.num_control_verts -= hull.pre_val2_interior as i32;
                    self.num_control_verts += hull.num_control_verts;
                }
            }
        }

        self.use_control_vert_map = self.control_faces_overlap;
        if self.use_control_vert_map {
            self.initialize_control_vertex_map();
        }
    }

    fn initialize_control_vertex_map(&mut self) {
        let face_size = self.surface.get_face_size() as usize;
        let base_verts = {
            let c0 = self.surface.get_corner_topology(0);
            let base_off = c0.get_face_index_offset(c0.get_face()) as usize;
            &self.surface.get_indices()[base_off..base_off + face_size]
        };
        // Add base face vertices — ensuring each slot is filled even if
        // indices repeat.
        for i in 0..face_size {
            let v = base_verts[i];
            if !self.control_vert_map.contains_key(&v) {
                let local = self.control_verts.len() as i32;
                self.control_vert_map.insert(v, local);
                self.control_verts.push(v);
            }
            if self.control_verts.len() == i {
                self.control_verts.push(v);
            }
        }

        for corner in 0..face_size {
            // Collect vertex indices into a local Vec first so we don't hold
            // an immutable borrow on `self` while mutating control_vert_map /
            // control_verts.
            let verts_to_add: Vec<Index> = {
                let hull  = &self.corner_hulls[corner];
                if hull.num_control_faces == 0 { continue; }

                let c_top = self.surface.get_corner_topology(corner);
                let c_sub = self.surface.get_corner_subset(corner);
                let idx   = self.get_corner_indices(corner);
                let mut verts = Vec::new();

                if hull.single_shared_face {
                    let nf = c_top.get_face_after(1);
                    let fv_off = c_top.get_face_index_offset(nf) as usize;
                    let s = c_top.get_face_size(nf) as usize;
                    for k in 1..s {
                        verts.push(idx[fv_off + k]);
                    }
                } else {
                    if c_sub.num_faces_after > 1 {
                        let mut nf = c_top.get_face_after(1);
                        for _ in 1..c_sub.num_faces_after {
                            nf = c_top.get_face_next(nf);
                            let fv_off = c_top.get_face_index_offset(nf) as usize;
                            let s = c_top.get_face_size(nf) as usize;
                            for k in 1..s { verts.push(idx[fv_off + k]); }
                        }
                    }
                    if c_sub.num_faces_before > 0 {
                        let mut nf = c_top.get_face_first(c_sub);
                        for _ in 0..c_sub.num_faces_before {
                            let fv_off = c_top.get_face_index_offset(nf) as usize;
                            let s = c_top.get_face_size(nf) as usize;
                            for k in 1..s { verts.push(idx[fv_off + k]); }
                            nf = c_top.get_face_next(nf);
                        }
                    }
                }
                verts
            };

            // Now mutate — no active borrows on `self` remain.
            for v in verts_to_add {
                if !self.control_vert_map.contains_key(&v) {
                    let local = self.control_verts.len() as i32;
                    self.control_vert_map.insert(v, local);
                    self.control_verts.push(v);
                }
            }
        }
        self.num_control_verts = self.control_verts.len() as i32;
    }

    // -----------------------------------------------------------------------
    //  Index helpers
    // -----------------------------------------------------------------------

    fn get_base_face_indices(&self) -> &[Index] {
        let c0 = self.surface.get_corner_topology(0);
        let base_off = c0.get_face_index_offset(c0.get_face()) as usize;
        let face_size = self.surface.get_face_size() as usize;
        &self.surface.get_indices()[base_off..base_off + face_size]
    }

    fn get_corner_indices(&self, corner: usize) -> &[Index] {
        let offset = self.corner_hulls[corner].surface_indices_offset as usize;
        &self.surface.get_indices()[offset..]
    }

    /// Translate a mesh-level vertex index to a local control hull index.
    /// Only valid when `use_control_vert_map` is true.
    fn get_local_control_vertex(&self, mesh_idx: Index) -> i32 {
        debug_assert!(self.use_control_vert_map);
        *self.control_vert_map.get(&mesh_idx)
            .expect("IrregularPatchBuilder: mesh index not in control vert map")
    }
}


// ---------------------------------------------------------------------------
//  Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn irreg_patch_options_default() {
        let opts = IrregPatchOptions::default();
        assert_eq!(opts.sharp_level, 6);
        assert_eq!(opts.smooth_level, 2);
        assert!(!opts.double_precision);
    }

    #[test]
    fn faces_match_identity() {
        assert!(IrregularPatchBuilder::faces_match(&[0, 1, 2, 3], &[0, 1, 2, 3]));
    }

    #[test]
    fn faces_match_rotation() {
        assert!(IrregularPatchBuilder::faces_match(&[0, 1, 2, 3], &[2, 3, 0, 1]));
    }

    #[test]
    fn faces_no_match() {
        assert!(!IrregularPatchBuilder::faces_match(&[0, 1, 2, 3], &[0, 1, 3, 2]));
    }
}
