//
// Factory for constructing a PatchTable from a TopologyRefiner.
// Contains the public PatchTableFactory API and internal PatchTableBuilder.

use super::patch_builder::{BasisType, PatchBuilder, PatchBuilderOptions, SingleCreaseInfo};
use super::patch_descriptor::{PatchDescriptor, PatchType};
use super::patch_param::PatchParam;
use super::patch_table::{FVarChannel, PatchArrayDescriptorInfo, PatchTable};
use super::ptex_indices::PtexIndices;
use super::sparse_matrix::SparseMatrix;
use super::stencil_table::StencilTable;
use super::topology_refiner::TopologyRefiner;
use super::types::{INDEX_INVALID, Index, index_is_valid};
use crate::sdc::options::FVarLinearInterpolation;
use crate::vtr::level::{Level, VSpan};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

#[inline]
fn is_sharpness_equal(a: f32, b: f32) -> bool {
    a == b
}

/// Assign or find an existing sharpness index in the dedup table.
fn assign_sharpness_index(sharpness: f32, values: &mut Vec<f32>) -> i32 {
    for (i, &v) in values.iter().enumerate() {
        if is_sharpness_equal(v, sharpness) {
            return i as i32;
        }
    }
    values.push(sharpness);
    (values.len() - 1) as i32
}

#[inline]
#[allow(dead_code)]
fn is_boundary_face(level: &Level, face: Index) -> bool {
    level.get_face_composite_vtag(face).boundary()
}

fn offset_indices(indices: &mut [Index], offset: i32) {
    for idx in indices.iter_mut() {
        *idx += offset;
    }
}

// ---------------------------------------------------------------------------
// EndCapType
// ---------------------------------------------------------------------------

/// Choice for approximating irregular patches (end-caps).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum EndCapType {
    None = 0,
    BilinearBasis = 1,
    BSplineBasis = 2,
    GregoryBasis = 3,
    LegacyGregory = 4,
}

impl Default for EndCapType {
    fn default() -> Self {
        EndCapType::GregoryBasis
    }
}

// ---------------------------------------------------------------------------
// PatchTableFactory::Options
// ---------------------------------------------------------------------------

/// Public options for PatchTableFactory.
/// Mirrors C++ `Far::PatchTableFactory::Options`.
#[derive(Debug, Clone)]
pub struct Options {
    pub generate_all_levels: bool,
    pub include_base_level_indices: bool,
    pub include_fvar_base_level_indices: bool,
    pub triangulate_quads: bool,
    pub use_single_crease_patch: bool,
    pub use_inf_sharp_patch: bool,
    pub max_isolation_level: u32,
    pub end_cap_type: EndCapType,
    pub share_end_cap_patch_points: bool,
    pub generate_varying_tables: bool,
    pub generate_varying_local_points: bool,
    pub generate_fvar_tables: bool,
    pub patch_precision_double: bool,
    pub fvar_patch_precision_double: bool,
    pub generate_fvar_legacy_linear_patches: bool,
    pub generate_legacy_sharp_corner_patches: bool,
    pub num_fvar_channels: i32,
    pub fvar_channel_indices: Option<Vec<i32>>,
}

impl Default for Options {
    fn default() -> Self {
        Self::new(10)
    }
}

impl Options {
    pub fn new(max_isolation: u32) -> Self {
        Self {
            generate_all_levels: false,
            include_base_level_indices: true,
            include_fvar_base_level_indices: false,
            triangulate_quads: false,
            use_single_crease_patch: false,
            use_inf_sharp_patch: false,
            max_isolation_level: max_isolation & 0xf,
            end_cap_type: EndCapType::GregoryBasis,
            share_end_cap_patch_points: true,
            generate_varying_tables: true,
            generate_varying_local_points: true,
            generate_fvar_tables: false,
            patch_precision_double: false,
            fvar_patch_precision_double: false,
            generate_fvar_legacy_linear_patches: true,
            generate_legacy_sharp_corner_patches: true,
            num_fvar_channels: -1,
            fvar_channel_indices: None,
        }
    }

    pub fn get_end_cap_type(&self) -> EndCapType {
        self.end_cap_type
    }

    /// Determine adaptive refinement options matching these patch options.
    pub fn get_refine_adaptive_options(&self) -> super::topology_refiner::AdaptiveOptions {
        let mut ao = super::topology_refiner::AdaptiveOptions::new(self.max_isolation_level);
        ao.use_inf_sharp_patch = self.use_inf_sharp_patch;
        ao.use_single_crease_patch = self.use_single_crease_patch;
        ao.consider_fvar_channels =
            self.generate_fvar_tables && !self.generate_fvar_legacy_linear_patches;
        ao
    }
}

// ---------------------------------------------------------------------------
// PatchTableFactory
// ---------------------------------------------------------------------------

/// Factory for constructing a PatchTable from a TopologyRefiner.
pub struct PatchTableFactory;

impl PatchTableFactory {
    /// Create a PatchTable from the given refiner and options.
    /// Optionally restrict to a subset of base faces via `selected_faces`.
    pub fn create(
        refiner: &TopologyRefiner,
        options: Options,
        selected_faces: &[Index],
    ) -> PatchTable {
        let mut builder = PatchTableBuilder::new(refiner, options, selected_faces);
        if builder.uniform_polygons_specified() {
            builder.build_uniform_polygons();
        } else {
            builder.build_patches();
        }
        builder.into_table()
    }
}

// ===========================================================================
//  PatchTableBuilder — internal implementation
// ===========================================================================

/// A <face, level> tuple identifying a single patch.
#[derive(Debug, Clone, Copy)]
struct PatchTuple {
    face_index: Index,
    level_index: i32,
}

/// Topological properties of a single patch, shared between vertex/fvar.
struct PatchInfo {
    is_regular: bool,
    is_reg_single_crease: bool,
    reg_boundary_mask: i32,
    reg_sharpness: f32,
    irreg_corner_spans: [VSpan; 4],
    param_boundary_mask: i32,
    f_matrix: SparseMatrix<f32>,
}

impl Default for PatchInfo {
    fn default() -> Self {
        Self {
            is_regular: false,
            is_reg_single_crease: false,
            reg_boundary_mask: 0,
            reg_sharpness: 0.0,
            irreg_corner_spans: Default::default(),
            param_boundary_mask: 0,
            f_matrix: SparseMatrix::default(),
        }
    }
}

// ---------------------------------------------------------------------------
// LocalPointStencilBuilder — accumulates stencil rows for endcap local points
// ---------------------------------------------------------------------------

/// Accumulates stencil data for the local (endcap) points produced by the
/// conversion matrix during adaptive patch building.
///
/// Mirrors the stencil-accumulation part of C++ `LocalPointHelper`.
struct LocalPointStencilBuilder {
    /// The stencil table being built (sizes/indices/weights).
    st: StencilTable,
    /// Absolute index of the first local point (= total refined vertex count).
    local_point_offset: i32,
    /// Number of local points accumulated so far.
    num_local_points: i32,
}

impl LocalPointStencilBuilder {
    fn new(local_point_offset: i32, expected_stencils: i32) -> Self {
        let mut st = StencilTable::new();
        st.reserve(expected_stencils, expected_stencils * 8);
        Self {
            st,
            local_point_offset,
            num_local_points: 0,
        }
    }
}

/// Internal builder that aggregates transient context during patch table
/// construction. Mirrors C++ `PatchTableBuilder` private class.
#[allow(dead_code)]
struct PatchTableBuilder<'a> {
    refiner: &'a TopologyRefiner,
    options: Options,
    selected_faces: Vec<Index>,

    // Derived flags
    requires_local_points: bool,
    requires_regular_local_points: bool,
    requires_irregular_local_points: bool,
    requires_sharpness_array: bool,
    requires_fvar_patches: bool,
    requires_varying_patches: bool,
    build_uniform_linear: bool,

    // The PatchTable under construction
    table: PatchTable,

    // PatchBuilder for topology queries
    patch_builder: PatchBuilder<'a>,
    ptex_indices: PtexIndices,

    // Identified patches
    patches: Vec<PatchTuple>,
    num_regular_patches: i32,
    num_irregular_patches: i32,

    // Per-level vertex/fvar-value offsets for index remapping
    level_vert_offsets: Vec<i32>,
    level_fvar_value_offsets: Vec<Vec<i32>>,
    fvar_channel_indices: Vec<i32>,
}

impl<'a> PatchTableBuilder<'a> {
    fn new(refiner: &'a TopologyRefiner, opts: Options, faces: &[Index]) -> Self {
        // Determine fvar channel indices
        let mut fvar_channel_indices = Vec::new();
        if opts.generate_fvar_tables {
            if opts.num_fvar_channels == -1 {
                let n = refiner.get_num_fvar_channels();
                fvar_channel_indices = (0..n).collect();
            } else if let Some(ref indices) = opts.fvar_channel_indices {
                fvar_channel_indices = indices.clone();
            } else {
                fvar_channel_indices = (0..opts.num_fvar_channels).collect();
            }
        }

        // Translate factory options to PatchBuilder options
        let mut pb_opts = PatchBuilderOptions::default();
        pb_opts.reg_basis = BasisType::Regular;
        pb_opts.irreg_basis = match opts.get_end_cap_type() {
            EndCapType::BilinearBasis => BasisType::Linear,
            EndCapType::BSplineBasis => BasisType::Regular,
            EndCapType::GregoryBasis => BasisType::Gregory,
            _ => BasisType::Unspecified,
        };
        pb_opts.fill_missing_boundary_points = true;
        pb_opts.approx_inf_sharp_with_smooth = !opts.use_inf_sharp_patch;
        pb_opts.approx_smooth_corner_with_sharp = opts.generate_legacy_sharp_corner_patches;

        let patch_builder = PatchBuilder::create(refiner, pb_opts);

        // Derived flags
        let requires_regular_local_points = false; // regBasis is always Regular
        let requires_irregular_local_points = opts.get_end_cap_type() != EndCapType::LegacyGregory;
        let requires_local_points =
            requires_irregular_local_points || requires_regular_local_points;
        let requires_sharpness_array = opts.use_single_crease_patch;
        let requires_fvar_patches = !fvar_channel_indices.is_empty();
        let requires_varying_patches = opts.generate_varying_tables;
        let build_uniform_linear = refiner.is_uniform(); // no non-linear uniform

        // Create and initialize the PatchTable
        let mut table = PatchTable::new();
        table.set_max_valence(refiner.get_max_valence());
        table.set_varying_desc(PatchDescriptor::new(patch_builder.get_linear_patch_type()));

        Self {
            refiner,
            options: opts,
            selected_faces: faces.to_vec(),
            requires_local_points,
            requires_regular_local_points,
            requires_irregular_local_points,
            requires_sharpness_array,
            requires_fvar_patches,
            requires_varying_patches,
            build_uniform_linear,
            table,
            patch_builder,
            ptex_indices: PtexIndices::new(refiner),
            patches: Vec::new(),
            num_regular_patches: 0,
            num_irregular_patches: 0,
            level_vert_offsets: Vec::new(),
            level_fvar_value_offsets: Vec::new(),
            fvar_channel_indices,
        }
    }

    fn uniform_polygons_specified(&self) -> bool {
        self.build_uniform_linear
    }

    fn into_table(self) -> PatchTable {
        self.table
    }

    fn get_refiner_fvar_channel(&self, fvc_in_table: i32) -> i32 {
        if fvc_in_table >= 0 {
            self.fvar_channel_indices[fvc_in_table as usize]
        } else {
            -1
        }
    }

    fn is_fvar_channel_linear(&self, fvc: i32) -> bool {
        if self.options.generate_fvar_legacy_linear_patches {
            return true;
        }
        let ch = self.get_refiner_fvar_channel(fvc);
        self.refiner.get_fvar_linear_interpolation(ch) == FVarLinearInterpolation::All
    }

    /// Check if fvar topology at this patch matches vertex topology for `fvc_in_table`.
    /// Mirrors C++ `doesFVarTopologyMatch`.
    fn does_fvar_topology_match(&self, patch: &PatchTuple, fvc_in_table: i32) -> bool {
        self.patch_builder.does_fvar_patch_match(
            patch.level_index,
            patch.face_index,
            self.get_refiner_fvar_channel(fvc_in_table),
        )
    }

    /// Identify fvar-channel-specific patch topology (non-linear path).
    /// Mirrors C++ `identifyPatchTopology(patch, info, fvcInTable)` with fvc >= 0.
    fn identify_fvar_patch_topology(&self, patch: &PatchTuple, info: &mut PatchInfo, fvc: i32) {
        let level_idx = patch.level_index;
        let face = patch.face_index;
        info.is_regular = self.patch_builder.is_patch_regular(level_idx, face, fvc);
        if info.is_regular {
            info.reg_boundary_mask = self
                .patch_builder
                .get_regular_patch_boundary_mask(level_idx, face, fvc);
            info.is_reg_single_crease = false;
            info.reg_sharpness = 0.0;
            info.param_boundary_mask = info.reg_boundary_mask;
        } else if self.requires_irregular_local_points {
            self.patch_builder.get_irregular_patch_corner_spans(
                level_idx,
                face,
                &mut info.irreg_corner_spans,
                fvc,
            );
            self.patch_builder.get_irregular_patch_conversion_matrix(
                level_idx,
                face,
                &info.irreg_corner_spans,
                &mut info.f_matrix,
            );
            info.param_boundary_mask = 0;
        }
    }

    /// Assign face-varying values for a patch (linear fvar path).
    /// Reads face fvar values at `fvc_in_table`, applies level offset, writes to `dest`.
    /// Mirrors C++ `assignFacePoints(patch, fptr[fvc], fvc)`.
    fn assign_fvar_face_points(
        &self,
        patch: &PatchTuple,
        dest: &mut [Index],
        fvc_in_table: i32,
    ) -> i32 {
        let fvc_refiner = self.get_refiner_fvar_channel(fvc_in_table);
        let level = self.refiner.get_level_internal(patch.level_index);
        let offset =
            self.level_fvar_value_offsets[fvc_in_table as usize][patch.level_index as usize];
        let fvals = level.get_face_fvar_values(patch.face_index, fvc_refiner);
        let n = fvals.size() as usize;
        for i in 0..n {
            dest[i] = fvals[i as i32] + offset;
        }
        n as i32
    }

    /// Assign fvar patch points (non-linear fvar path).
    /// Source offsets come from `level_fvar_value_offsets`.
    /// Mirrors C++ `assignPatchPointsAndStencils(patch, fvcPatchInfo, fptr[fvc], helper, fvc)`.
    fn assign_fvar_patch_points(
        &self,
        patch: &PatchTuple,
        info: &PatchInfo,
        dest: &mut [Index],
        stencil_builder: Option<&mut LocalPointStencilBuilder>,
        fvc_in_table: i32,
    ) -> i32 {
        let fvc_refiner = self.get_refiner_fvar_channel(fvc_in_table);
        let source_offset =
            self.level_fvar_value_offsets[fvc_in_table as usize][patch.level_index as usize];

        if info.is_regular {
            let n = self.patch_builder.get_regular_patch_points(
                patch.level_index,
                patch.face_index,
                info.reg_boundary_mask,
                dest,
                fvc_refiner,
            );
            offset_indices(&mut dest[..n as usize], source_offset);
            n
        } else if self.requires_irregular_local_points {
            let num_src = info.f_matrix.get_num_columns();
            let num_pts = info.f_matrix.get_num_rows();
            let mut src = vec![0i32; num_src as usize];
            self.patch_builder.get_irregular_patch_source_points(
                patch.level_index,
                patch.face_index,
                &info.irreg_corner_spans,
                &mut src,
                fvc_refiner,
            );

            if let Some(builder) = stencil_builder {
                let first_local = builder.num_local_points;
                for row in 0..num_pts {
                    let sz = info.f_matrix.get_row_size(row) as usize;
                    let cols = info.f_matrix.get_row_columns(row);
                    let weights = info.f_matrix.get_row_elements(row);
                    builder.st.sizes.push(sz as i32);
                    for k in 0..sz {
                        builder
                            .st
                            .indices
                            .push(src[cols[k] as usize] + source_offset);
                        builder.st.weights.push(weights[k]);
                    }
                    dest[row as usize] = builder.local_point_offset + first_local + row;
                }
                builder.num_local_points += num_pts;
            } else {
                for i in 0..num_pts.min(dest.len() as i32) {
                    dest[i as usize] = if i < num_src {
                        src[i as usize] + source_offset
                    } else {
                        INDEX_INVALID
                    };
                }
            }
            num_pts
        } else {
            self.assign_fvar_face_points(patch, dest, fvc_in_table)
        }
    }

    /// Allocate fvar channels in the patch table (adaptive path).
    /// Mirrors C++ `PatchTableBuilder::allocateFVarChannels()`.
    fn allocate_fvar_channels(&mut self, total_patches: i32) {
        let n_fvar = self.fvar_channel_indices.len();
        for fvc in 0..n_fvar {
            let refiner_channel = self.fvar_channel_indices[fvc];
            let linear_type = self.patch_builder.get_linear_patch_type();

            let (reg_type, irreg_type) = if self.is_fvar_channel_linear(fvc as i32) {
                (linear_type, linear_type)
            } else {
                (
                    self.patch_builder.get_regular_patch_type(),
                    self.patch_builder.get_irregular_patch_type(),
                )
            };

            let reg_ncv = PatchDescriptor::new(reg_type).get_num_control_vertices();
            let irreg_ncv = PatchDescriptor::new(irreg_type).get_num_control_vertices();
            let stride = reg_ncv.max(irreg_ncv);

            // Store linear interpolation mode (mirrors C++ setFVarPatchChannelLinearInterpolation).
            let interp = self.refiner.get_fvar_linear_interpolation(refiner_channel);

            let ch = FVarChannel {
                regular_desc: PatchDescriptor::new(reg_type),
                irregular_desc: PatchDescriptor::new(irreg_type),
                stride,
                interpolation: interp,
                vertices: vec![0i32; (total_patches * stride) as usize],
                params: vec![PatchParam::default(); total_patches as usize],
            };
            self.table.fvar_channels.push(ch);
        }
    }

    // -----------------------------------------------------------------------
    //  Build uniform polygons
    // -----------------------------------------------------------------------

    fn build_uniform_polygons(&mut self) {
        let include_base_indices = self.options.include_base_level_indices;
        let include_base_fvar = self.options.include_fvar_base_level_indices;
        let triangulate =
            self.options.triangulate_quads && self.patch_builder.get_regular_face_size() == 4;

        let max_level = self.refiner.get_max_level();
        let first_level = if self.options.generate_all_levels {
            1
        } else {
            max_level
        };
        let n_levels = (max_level - first_level + 1) as usize;

        let ptype = if triangulate {
            PatchType::Triangles
        } else {
            self.patch_builder.get_linear_patch_type()
        };

        let desc = PatchDescriptor::new(ptype);
        let ncv = desc.get_num_control_vertices();

        // Count patches at each level and build patch arrays
        let mut patch_arrays = Vec::with_capacity(n_levels);
        let mut total_patches = 0i32;
        let mut total_cvs = 0i32;

        for level in first_level..=max_level {
            let ref_level = self.refiner.get_level(level);
            let mut npatches = ref_level.get_num_faces();
            if self.refiner.has_holes() {
                for i in (0..npatches).rev() {
                    if ref_level.is_face_hole(i) {
                        npatches -= 1;
                    }
                }
            }
            if triangulate {
                npatches *= 2;
            }

            patch_arrays.push(PatchArrayDescriptorInfo {
                descriptor: desc,
                num_patches: npatches,
                index_base: total_cvs,
                patch_index_base: total_patches,
                // Uniform polygons never use the Gregory quad-offset table.
                quad_offset_index: 0,
            });
            total_cvs += npatches * ncv;
            total_patches += npatches;
        }

        for pa in &patch_arrays {
            self.table.push_patch_array(pa.clone());
        }

        // Allocate buffers
        let mut all_verts: Vec<Index> = vec![0; total_cvs as usize];
        let mut all_params: Vec<PatchParam> = vec![PatchParam::default(); total_patches as usize];
        // Sharpness not needed for uniform

        // FVar channels
        let n_fvar = self.fvar_channel_indices.len();
        let mut fvar_verts: Vec<Vec<Index>> = vec![vec![0; total_cvs as usize]; n_fvar];
        let mut fvar_params: Vec<Vec<PatchParam>> =
            vec![vec![PatchParam::default(); total_patches as usize]; n_fvar];

        let mut fvar_vert_offsets: Vec<i32> = vec![0; n_fvar];
        if include_base_fvar {
            for (i, &fvc) in self.fvar_channel_indices.iter().enumerate() {
                fvar_vert_offsets[i] = self.refiner.get_level(0).get_num_fvar_values(fvc);
            }
        }

        // Populate
        let mut iptr = 0usize;
        let mut pptr = 0usize;
        let mut fptrs: Vec<usize> = vec![0; n_fvar];
        let mut fpptrs: Vec<usize> = vec![0; n_fvar];

        let mut level_vert_offset: i32 = if include_base_indices {
            self.refiner.get_level(0).get_num_vertices()
        } else {
            0
        };

        for level in 1..=max_level {
            let ref_level = self.refiner.get_level(level);
            let n_faces = ref_level.get_num_faces();

            if level >= first_level {
                for face in 0..n_faces {
                    if self.refiner.has_holes() && ref_level.is_face_hole(face) {
                        continue;
                    }

                    let fverts = ref_level.get_face_vertices(face);
                    for vi in 0..fverts.size() {
                        all_verts[iptr] = level_vert_offset + fverts[vi];
                        iptr += 1;
                    }

                    let pparam = self.patch_builder.compute_patch_param(
                        level,
                        face,
                        &self.ptex_indices,
                        true,
                        0,
                        false,
                    );
                    all_params[pptr] = pparam;
                    pptr += 1;

                    // FVar
                    for (fi, &fvc) in self.fvar_channel_indices.iter().enumerate() {
                        let fvals = ref_level.get_face_fvar_values(face, fvc);
                        for vi in 0..fvals.size() {
                            fvar_verts[fi][fptrs[fi]] = fvar_vert_offsets[fi] + fvals[vi];
                            fptrs[fi] += 1;
                        }
                        fvar_params[fi][fpptrs[fi]] = pparam;
                        fpptrs[fi] += 1;
                    }

                    if triangulate {
                        // {v0,v1,v2,v3} -> second tri: {v3,v0,v2}
                        all_verts[iptr] = all_verts[iptr - 4]; // v0
                        all_verts[iptr + 1] = all_verts[iptr - 2]; // v2
                        iptr += 2;
                        all_params[pptr] = pparam;
                        pptr += 1;

                        for fi in 0..n_fvar {
                            let fp = fptrs[fi];
                            fvar_verts[fi][fp] = fvar_verts[fi][fp - 4];
                            fvar_verts[fi][fp + 1] = fvar_verts[fi][fp - 2];
                            fptrs[fi] += 2;
                            fvar_params[fi][fpptrs[fi]] = pparam;
                            fpptrs[fi] += 1;
                        }
                    }
                }
            }

            if self.options.generate_all_levels {
                level_vert_offset += self.refiner.get_level(level).get_num_vertices();
                for (fi, &fvc) in self.fvar_channel_indices.iter().enumerate() {
                    fvar_vert_offsets[fi] += self.refiner.get_level(level).get_num_fvar_values(fvc);
                }
            }
        }

        // Store into table
        self.table.push_vertices(&all_verts);
        self.table.push_patch_params(&all_params);

        // FVar channels
        for (fi, &fvc) in self.fvar_channel_indices.iter().enumerate() {
            let reg_desc = desc;
            let irreg_desc = desc;
            let interpolation = self.refiner.get_fvar_linear_interpolation(fvc);
            let _ = interpolation; // stored in C++ but not needed here

            let mut ch = FVarChannel::default();
            ch.regular_desc = reg_desc;
            ch.irregular_desc = irreg_desc;
            ch.stride = ncv;
            ch.vertices = fvar_verts[fi].clone();
            ch.params = fvar_params[fi].clone();
            self.table.fvar_channels.push(ch);
        }
    }

    // -----------------------------------------------------------------------
    //  Build adaptive patches
    // -----------------------------------------------------------------------

    fn build_patches(&mut self) {
        self.identify_patches();
        self.populate_patches();
    }

    fn append_patch(&mut self, level_index: i32, face_index: Index) {
        self.patches.push(PatchTuple {
            face_index,
            level_index,
        });
        if self
            .patch_builder
            .is_patch_regular(level_index, face_index, -1)
        {
            self.num_regular_patches += 1;
        } else {
            self.num_irregular_patches += 1;
        }
    }

    fn identify_patches(&mut self) {
        // Initialize level offsets
        self.level_vert_offsets.push(0);
        self.level_fvar_value_offsets
            .resize(self.fvar_channel_indices.len(), Vec::new());
        for fvc_offsets in &mut self.level_fvar_value_offsets {
            fvc_offsets.push(0);
        }

        for li in 0..self.refiner.get_num_levels() {
            let level = self.refiner.get_level_internal(li);
            self.level_vert_offsets
                .push(*self.level_vert_offsets.last().unwrap() + level.get_num_vertices());
            for (fi, &fvc) in self.fvar_channel_indices.clone().iter().enumerate() {
                let prev = *self.level_fvar_value_offsets[fi].last().unwrap();
                self.level_fvar_value_offsets[fi].push(prev + level.get_num_fvar_values(fvc));
            }
        }

        // Identify patches
        let uniform_level = if self.refiner.is_uniform() {
            self.options.max_isolation_level as i32
        } else {
            -1
        };

        let total_faces = self.refiner.get_num_faces_total();
        self.patches.reserve(total_faces as usize);

        if !self.selected_faces.is_empty() {
            // Depth-first from selected base faces
            let sel = self.selected_faces.clone();
            for &f in &sel {
                self.find_descendant_patches(0, f, uniform_level);
            }
        } else if uniform_level >= 0 {
            let n = self
                .refiner
                .get_level_internal(uniform_level)
                .get_num_faces();
            for fi in 0..n {
                if self.patch_builder.is_face_a_patch(uniform_level, fi) {
                    self.append_patch(uniform_level, fi);
                }
            }
        } else {
            // Breadth-first
            for li in 0..self.refiner.get_num_levels() {
                let n = self.refiner.get_level_internal(li).get_num_faces();
                for fi in 0..n {
                    if self.patch_builder.is_face_a_patch(li, fi)
                        && self.patch_builder.is_face_a_leaf(li, fi)
                    {
                        self.append_patch(li, fi);
                    }
                }
            }
        }
    }

    fn find_descendant_patches(&mut self, level_index: i32, face_index: Index, target_level: i32) {
        if (level_index == target_level)
            || self.patch_builder.is_face_a_leaf(level_index, face_index)
        {
            if self.patch_builder.is_face_a_patch(level_index, face_index) {
                self.append_patch(level_index, face_index);
            }
        } else {
            let level = self.refiner.get_level(level_index);
            let child_faces = level.get_face_child_faces(face_index);
            let children: Vec<Index> = (0..child_faces.size()).map(|i| child_faces[i]).collect();
            for cf in children {
                if index_is_valid(cf) {
                    self.find_descendant_patches(level_index + 1, cf, target_level);
                }
            }
        }
    }

    fn identify_patch_topology(&self, patch: &PatchTuple, info: &mut PatchInfo) {
        let level_idx = patch.level_index;
        let face = patch.face_index;

        info.is_regular = self.patch_builder.is_patch_regular(level_idx, face, -1);

        if info.is_regular {
            info.reg_boundary_mask = self
                .patch_builder
                .get_regular_patch_boundary_mask(level_idx, face, -1);

            info.is_reg_single_crease = false;
            info.reg_sharpness = 0.0;
            info.param_boundary_mask = info.reg_boundary_mask;

            // Single crease detection for interior regular patches
            if self.requires_sharpness_array && info.reg_boundary_mask == 0 {
                if level_idx < self.options.max_isolation_level as i32 {
                    let mut crease_info = SingleCreaseInfo::default();
                    if self.patch_builder.is_regular_single_crease_patch(
                        level_idx,
                        face,
                        &mut crease_info,
                    ) {
                        crease_info.crease_sharpness = crease_info
                            .crease_sharpness
                            .min((self.options.max_isolation_level as i32 - level_idx) as f32);
                        info.is_reg_single_crease = true;
                        info.reg_sharpness = crease_info.crease_sharpness;
                        info.param_boundary_mask = 1 << crease_info.crease_edge_in_face;
                    }
                }
            }
        } else if self.requires_irregular_local_points {
            self.patch_builder.get_irregular_patch_corner_spans(
                level_idx,
                face,
                &mut info.irreg_corner_spans,
                -1,
            );

            self.patch_builder.get_irregular_patch_conversion_matrix(
                level_idx,
                face,
                &info.irreg_corner_spans,
                &mut info.f_matrix,
            );

            info.param_boundary_mask = 0;
        }
    }

    /// Assign patch point indices for a single patch.
    ///
    /// For irregular patches this also appends stencil rows to `stencil_builder`
    /// (when provided) and writes sequential local-point indices starting at
    /// `local_point_offset`.  Mirrors C++ `assignPatchPointsAndStencils`.
    ///
    /// Returns the number of patch control points written.
    fn assign_patch_points(
        &self,
        patch: &PatchTuple,
        info: &PatchInfo,
        dest: &mut [Index],
        stencil_builder: Option<&mut LocalPointStencilBuilder>,
    ) -> i32 {
        let source_offset = self.level_vert_offsets[patch.level_index as usize];

        if info.is_regular {
            let n = self.patch_builder.get_regular_patch_points(
                patch.level_index,
                patch.face_index,
                info.reg_boundary_mask,
                dest,
                -1,
            );
            offset_indices(&mut dest[..n as usize], source_offset);
            n
        } else if self.requires_irregular_local_points {
            let num_src = info.f_matrix.get_num_columns();
            let num_pts = info.f_matrix.get_num_rows();
            let mut src = vec![0i32; num_src as usize];
            self.patch_builder.get_irregular_patch_source_points(
                patch.level_index,
                patch.face_index,
                &info.irreg_corner_spans,
                &mut src,
                -1,
            );

            if let Some(builder) = stencil_builder {
                // Append one stencil per row of the conversion matrix.
                // Each stencil maps source points (with offset) to one local point.
                // Mirrors C++ LocalPointHelper::appendLocalPointStencils.
                let first_local = builder.num_local_points;
                for row in 0..num_pts {
                    let sz = info.f_matrix.get_row_size(row) as usize;
                    let cols = info.f_matrix.get_row_columns(row);
                    let weights = info.f_matrix.get_row_elements(row);
                    builder.st.sizes.push(sz as i32);
                    for k in 0..sz {
                        builder
                            .st
                            .indices
                            .push(src[cols[k] as usize] + source_offset);
                        builder.st.weights.push(weights[k]);
                    }
                    // patch point = local_point_offset + running index
                    dest[row as usize] = builder.local_point_offset + first_local + row;
                }
                builder.num_local_points += num_pts;
            } else {
                // No stencil builder: fall back to writing source-point indices
                // (correct for topology, no stencil evaluation).
                for i in 0..num_pts.min(dest.len() as i32) {
                    dest[i as usize] = if i < num_src {
                        src[i as usize] + source_offset
                    } else {
                        INDEX_INVALID
                    };
                }
            }
            num_pts
        } else {
            // Legacy Gregory: just assign face vertices
            self.assign_face_points(patch, dest)
        }
    }

    /// Assign face vertex indices for a patch (linear/legacy path).
    fn assign_face_points(&self, patch: &PatchTuple, dest: &mut [Index]) -> i32 {
        let level = self.refiner.get_level_internal(patch.level_index);
        let offset = self.level_vert_offsets[patch.level_index as usize];
        let fverts = level.get_face_vertices(patch.face_index);
        for i in 0..fverts.size() as usize {
            dest[i] = fverts[i as i32] + offset;
        }
        fverts.size()
    }

    fn populate_patches(&mut self) {
        let reg_type = self.patch_builder.get_regular_patch_type();
        let irreg_type = self.patch_builder.get_irregular_patch_type();

        // local_point_offset = total number of refined vertices across all levels.
        // Local points are indexed starting from this value.
        // level_vert_offsets has one extra sentinel entry (last = total verts).
        let local_point_offset: i32 = *self.level_vert_offsets.last().unwrap_or(&0);

        // Determine patch arrays
        let same_type = reg_type == irreg_type;
        let mut num_arrays = 0;
        let array_regular;
        let array_irregular;

        if self.num_regular_patches > 0 {
            array_regular = 0;
            num_arrays = 1;
        } else {
            array_regular = 0;
        }

        if self.num_irregular_patches > 0 {
            if same_type {
                array_irregular = array_regular;
                if num_arrays == 0 {
                    num_arrays = 1;
                }
            } else {
                array_irregular = num_arrays;
                num_arrays += 1;
            }
        } else {
            array_irregular = 0;
        }

        // Build PatchArrayDescriptorInfos
        #[derive(Clone)]
        struct ArrayInfo {
            patch_type: PatchType,
            num_patches: i32,
        }
        let mut array_infos = vec![
            ArrayInfo {
                patch_type: PatchType::NonPatch,
                num_patches: 0
            };
            num_arrays as usize
        ];

        if self.num_regular_patches > 0 {
            array_infos[array_regular as usize].patch_type = reg_type;
            array_infos[array_regular as usize].num_patches += self.num_regular_patches;
        }
        if self.num_irregular_patches > 0 {
            array_infos[array_irregular as usize].patch_type = irreg_type;
            array_infos[array_irregular as usize].num_patches += self.num_irregular_patches;
        }

        // Push patch arrays into table
        let mut voffset = 0i32;
        let mut poffset = 0i32;
        for ai in &array_infos {
            let desc = PatchDescriptor::new(ai.patch_type);
            let ncv = desc.get_num_control_vertices();
            self.table.push_patch_array(PatchArrayDescriptorInfo {
                descriptor: desc,
                num_patches: ai.num_patches,
                index_base: voffset,
                patch_index_base: poffset,
                // Quad-offset table population is deferred to legacy Gregory build.
                // For non-legacy paths this stays 0.
                quad_offset_index: 0,
            });
            voffset += ai.num_patches * ncv;
            poffset += ai.num_patches;
        }

        // Allocate vertex + param buffers
        let total_cvs = voffset;
        let total_patches = poffset;
        let mut all_verts = vec![0i32; total_cvs as usize];
        let mut all_params = vec![PatchParam::default(); total_patches as usize];
        let mut all_sharp = if self.requires_sharpness_array {
            vec![INDEX_INVALID; total_patches as usize]
        } else {
            Vec::new()
        };

        // Per-array write cursors: (vert_cursor, param_cursor)
        let mut cursors: Vec<(usize, usize)> = Vec::with_capacity(num_arrays as usize);
        for ai_idx in 0..num_arrays as usize {
            let pa = &self.table.patch_arrays[ai_idx];
            cursors.push((pa.index_base as usize, pa.patch_index_base as usize));
        }

        // Build sharpness values dedup table
        let mut sharpness_values: Vec<f32> = Vec::new();

        // Local-point stencil builder — only allocated when end-cap type needs it.
        // Mirrors C++ LocalPointHelper (without point-sharing for now).
        let mut stencil_builder: Option<LocalPointStencilBuilder> =
            if self.requires_irregular_local_points && self.num_irregular_patches > 0 {
                let expected = self.num_irregular_patches * 20; // conservative estimate
                Some(LocalPointStencilBuilder::new(local_point_offset, expected))
            } else {
                None
            };

        // Allocate fvar channels now that total_patches is known.
        // Mirrors C++ lines 1107-1109: if (_requiresFVarPatches) allocateFVarChannels().
        if self.requires_fvar_patches {
            self.allocate_fvar_channels(total_patches);
        }

        // Per-fvar-channel state for the patch loop.
        let n_fvar = self.fvar_channel_indices.len();
        // Per-channel stencil builders for fvar local points (one per channel).
        let mut fvar_stencil_builders: Vec<Option<LocalPointStencilBuilder>> =
            if self.requires_fvar_patches && self.requires_irregular_local_points {
                (0..n_fvar)
                    .map(|fvc| {
                        let fvc_refiner = self.fvar_channel_indices[fvc];
                        // Total fvar values across all levels is the local-point base offset
                        // for this channel. Mirrors C++ fvarLocalPointOffset computation.
                        let fvar_lp_offset = self.refiner.get_num_fvar_values_total(fvc_refiner);
                        Some(LocalPointStencilBuilder::new(
                            fvar_lp_offset,
                            self.num_irregular_patches * 20,
                        ))
                    })
                    .collect()
            } else {
                (0..n_fvar).map(|_| None).collect()
            };
        // Monotone cursor into fvar_channels arrays (one slot per patch, in patch-loop order).
        let mut fvar_patch_cursor: usize = 0;

        // Populate each patch
        let patches = self.patches.clone();
        let mut patch_info = PatchInfo::default();

        for patch in &patches {
            self.identify_patch_topology(patch, &mut patch_info);

            let arr_idx = if patch_info.is_regular {
                array_regular as usize
            } else {
                array_irregular as usize
            };

            let (ref mut vcur, ref mut pcur) = cursors[arr_idx];

            // Assign patch points (and accumulate stencils for irregular patches)
            let desc = array_infos[arr_idx].patch_type;
            let ncv = PatchDescriptor::new(desc).get_num_control_vertices() as usize;
            let n = self.assign_patch_points(
                patch,
                &patch_info,
                &mut all_verts[*vcur..*vcur + ncv],
                stencil_builder.as_mut(),
            );
            *vcur += n as usize;

            // Sharpness
            if self.requires_sharpness_array {
                all_sharp[*pcur] =
                    assign_sharpness_index(patch_info.reg_sharpness, &mut sharpness_values);
            }

            // PatchParam
            let pparam = self.patch_builder.compute_patch_param(
                patch.level_index,
                patch.face_index,
                &self.ptex_indices,
                patch_info.is_regular,
                patch_info.param_boundary_mask,
                patch.level_index < self.refiner.get_max_level(),
            );
            all_params[*pcur] = pparam;
            *pcur += 1;

            // ----------------------------------------------------------------
            // Face-varying patch assignment.
            // Mirrors C++ populatePatches() lines ~1226-1268.
            // ----------------------------------------------------------------
            if self.requires_fvar_patches {
                // Whether vertex and fvar precision modes match (both double or both float).
                // When they match, topology comparison is valid.
                let fvar_precision_matches =
                    self.options.patch_precision_double == self.options.fvar_patch_precision_double;

                for fvc in 0..n_fvar {
                    let stride = self.table.fvar_channels[fvc].stride as usize;
                    let vert_base = fvar_patch_cursor * stride;

                    // Write into a temp buffer to avoid simultaneous &self / &mut self.table borrow.
                    let mut fvar_pts = vec![0i32; stride];

                    if self.is_fvar_channel_linear(fvc as i32) {
                        // Linear fvar: just copy the face quad corner values.
                        self.assign_fvar_face_points(patch, &mut fvar_pts, fvc as i32);
                        self.table.fvar_channels[fvc].vertices[vert_base..vert_base + stride]
                            .copy_from_slice(&fvar_pts);
                        // Reuse the vertex PatchParam as-is for linear channels.
                        self.table.fvar_channels[fvc].params[fvar_patch_cursor] = pparam;
                    } else {
                        // Non-linear fvar: determine if fvar topology matches vertex topology.
                        let topo_matches = fvar_precision_matches
                            && self.does_fvar_topology_match(patch, fvc as i32);

                        // If topology matches we reuse the already-computed patch_info;
                        // otherwise we identify fvar-specific topology into a temp.
                        let fvc_info_tmp;
                        let fvc_info: &PatchInfo = if topo_matches {
                            &patch_info
                        } else {
                            let mut tmp = PatchInfo::default();
                            self.identify_fvar_patch_topology(patch, &mut tmp, fvc as i32);
                            fvc_info_tmp = tmp;
                            &fvc_info_tmp
                        };

                        // Assign patch points (stencil builder is per-channel).
                        let fvar_builder = &mut fvar_stencil_builders[fvc];
                        self.assign_fvar_patch_points(
                            patch,
                            fvc_info,
                            &mut fvar_pts,
                            fvar_builder.as_mut(),
                            fvc as i32,
                        );
                        self.table.fvar_channels[fvc].vertices[vert_base..vert_base + stride]
                            .copy_from_slice(&fvar_pts);

                        // Build fvar-specific PatchParam with fvar boundary mask.
                        let mut fvc_param = PatchParam::default();
                        fvc_param.set(
                            pparam.get_face_id(),
                            pparam.get_u() as i16,
                            pparam.get_v() as i16,
                            pparam.get_depth() as u16,
                            pparam.non_quad_root(),
                            fvc_info.param_boundary_mask as u16,
                            pparam.get_transition() as u16,
                            fvc_info.is_regular,
                        );
                        self.table.fvar_channels[fvc].params[fvar_patch_cursor] = fvc_param;
                    }
                }
                fvar_patch_cursor += 1;
            }
        }

        // Finalize table buffers
        self.table.push_vertices(&all_verts);
        self.table.push_patch_params(&all_params);

        if self.requires_sharpness_array {
            self.table.sharpness_indices = all_sharp;
            self.table.sharpness_values = sharpness_values;
        }

        // Finalize and store local-point stencil table.
        // Mirrors C++: `_table->_localPointStencils = vertexLocalPointHelper->AcquireStencilTable()`
        if let Some(mut builder) = stencil_builder {
            if builder.num_local_points > 0 {
                builder.st.finalize();
                builder.st.set_num_control_vertices(local_point_offset);
                self.table
                    .set_local_point_stencil_table(Box::new(builder.st));
            }
        }

        // Finalize per-channel fvar local-point stencil tables.
        // Mirrors C++ lines 1287-1295: fvarLocalPointHelper->AcquireStencilTable() per channel.
        for (fvc, fvar_builder_opt) in fvar_stencil_builders.into_iter().enumerate() {
            if let Some(mut builder) = fvar_builder_opt {
                if builder.num_local_points > 0 {
                    let fvc_refiner = self.fvar_channel_indices[fvc];
                    let fvar_lp_offset = self.refiner.get_num_fvar_values_total(fvc_refiner);
                    builder.st.finalize();
                    builder.st.set_num_control_vertices(fvar_lp_offset);
                    if fvc < self.table.local_point_fvar_stencil_tables.len() {
                        self.table.local_point_fvar_stencil_tables[fvc] =
                            Some(Box::new(builder.st));
                    } else {
                        // Channel was appended by allocate_fvar_channels — push to match.
                        while self.table.local_point_fvar_stencil_tables.len() < fvc {
                            self.table.local_point_fvar_stencil_tables.push(None);
                        }
                        self.table
                            .local_point_fvar_stencil_tables
                            .push(Some(Box::new(builder.st)));
                    }
                }
            }
        }

        // Populate varying vertices.
        //
        // Mirrors C++ PatchTableBuilder::populateVaryingVertices():
        // for each patch, write the regular face corner vertices into the
        // varying table (one varying quad per patch, 4 entries each).
        // The varying_desc is set to QUADS in PatchTable::new(), so nvcv == 4.
        if self.requires_varying_patches {
            let nvcv = self.table.varying_desc.get_num_control_vertices() as usize;
            if nvcv > 0 {
                let num_patches = total_patches as usize;
                let mut var_verts = vec![0i32; num_patches * nvcv];

                for (pi, patch) in patches.iter().enumerate() {
                    let level = self.refiner.get_level_internal(patch.level_index);
                    let offset = self.level_vert_offsets[patch.level_index as usize];
                    let fverts = level.get_face_vertices(patch.face_index);
                    let n_fv = fverts.size() as usize;
                    let base = pi * nvcv;
                    // Write up to nvcv face-corner vertices as varying
                    for k in 0..nvcv.min(n_fv) {
                        var_verts[base + k] = fverts[k as i32] + offset;
                    }
                    // If face has fewer verts than nvcv (triangle in quad scheme), repeat last
                    for k in n_fv..nvcv {
                        var_verts[base + k] = if n_fv > 0 {
                            fverts[(n_fv - 1) as i32] + offset
                        } else {
                            INDEX_INVALID
                        };
                    }
                }

                self.table.varying_vertices = var_verts;
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn options_default() {
        let opts = Options::default();
        assert_eq!(opts.max_isolation_level, 10);
        assert_eq!(opts.end_cap_type, EndCapType::GregoryBasis);
        assert!(opts.include_base_level_indices);
        assert!(opts.generate_varying_tables);
        assert!(!opts.generate_fvar_tables);
    }

    #[test]
    fn options_new_clamps() {
        let opts = Options::new(20);
        // max_isolation_level is clamped to 4 bits (0..15)
        assert!(opts.max_isolation_level <= 15);
    }

    #[test]
    fn end_cap_type_default() {
        assert_eq!(EndCapType::default(), EndCapType::GregoryBasis);
    }

    #[test]
    fn sharpness_index_dedup() {
        let mut values = Vec::new();
        assert_eq!(assign_sharpness_index(1.0, &mut values), 0);
        assert_eq!(assign_sharpness_index(2.0, &mut values), 1);
        assert_eq!(assign_sharpness_index(1.0, &mut values), 0);
        assert_eq!(values.len(), 2);
    }

    #[test]
    fn refine_adaptive_options() {
        let mut opts = Options::default();
        opts.use_inf_sharp_patch = true;
        opts.use_single_crease_patch = true;
        opts.generate_fvar_tables = true;
        opts.generate_fvar_legacy_linear_patches = false;

        let ao = opts.get_refine_adaptive_options();
        assert!(ao.use_inf_sharp_patch);
        assert!(ao.use_single_crease_patch);
        assert!(ao.consider_fvar_channels);
    }

    #[test]
    fn create_empty_refiner() {
        // Create a trivial refiner with no mesh data -- factory should
        // produce a table without panicking. Uniform path produces one
        // patch array (the last level) even if it has 0 patches.
        use crate::sdc::{Options as SdcOpts, types::SchemeType};
        let refiner = TopologyRefiner::new(SchemeType::Catmark, SdcOpts::default());
        let table = PatchTableFactory::create(&refiner, Options::default(), &[]);
        // No crash = success; patch count at array 0 should be 0
        if table.get_num_patch_arrays() > 0 {
            assert_eq!(table.get_num_patches(0), 0);
        }
    }
}
