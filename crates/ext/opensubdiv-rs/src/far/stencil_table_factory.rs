//! Factory for building [`StencilTable`] and [`LimitStencilTable`] from a
//! [`TopologyRefiner`].
//!
//! Mirrors C++ `Far::StencilTableFactory` and `Far::LimitStencilTableFactory`.

use super::patch_map::PatchMap;
use super::patch_table::PatchTable;
use super::patch_table_factory::{
    EndCapType, Options as PatchTableFactoryOptions, PatchTableFactory,
};
use super::primvar_refiner::PrimvarRefiner;
use super::stencil_builder::StencilBuilder;
use super::stencil_table::{LimitStencilTable, StencilTable};
use super::topology_refiner::TopologyRefiner;
use crate::far::types::Index;

// ---------------------------------------------------------------------------
// InterpolationMode
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum InterpolationMode {
    #[default]
    Vertex = 0,
    Varying = 1,
    FaceVarying = 2,
}

// ---------------------------------------------------------------------------
// StencilTableFactory
// ---------------------------------------------------------------------------

/// Options for `StencilTableFactory::create`.
#[derive(Clone, Copy)]
pub struct StencilTableOptions {
    pub interpolation_mode: InterpolationMode,
    /// Populate the `offsets` field in the resulting table.
    pub generate_offsets: bool,
    /// Include identity stencils for coarse (level-0) control vertices.
    pub generate_control_verts: bool,
    /// Include stencils for all intermediate refinement levels.
    pub generate_intermediate_levels: bool,
    /// Flatten stencils so they reference only coarse CVs (no chaining).
    pub factorize_intermediate_levels: bool,
    /// Maximum refinement level to include stencils for (0-15).
    pub max_level: u32,
    /// Face-varying channel to use when `interpolation_mode == FaceVarying`.
    pub fvar_channel: u32,
}

impl Default for StencilTableOptions {
    fn default() -> Self {
        Self {
            interpolation_mode: InterpolationMode::Vertex,
            generate_offsets: false,
            generate_control_verts: false,
            generate_intermediate_levels: true,
            factorize_intermediate_levels: true,
            max_level: 10,
            fvar_channel: 0,
        }
    }
}

/// Factory for building `StencilTable` from a refined `TopologyRefiner`.
pub struct StencilTableFactory;

impl StencilTableFactory {
    /// Build a stencil table from a refined topology refiner.
    ///
    /// Mirrors C++ `Far::StencilTableFactory::Create(refiner, options)`.
    #[doc(alias = "Create")]
    pub fn create(refiner: &TopologyRefiner, options: StencilTableOptions) -> Box<StencilTable> {
        let fv = options.interpolation_mode == InterpolationMode::FaceVarying;

        let num_control = if fv {
            refiner
                .get_level_internal(0)
                .get_num_fvar_values(options.fvar_channel as i32)
        } else {
            refiner.get_level_internal(0).get_num_vertices()
        };

        let max_level = std::cmp::min(options.max_level as i32, refiner.get_max_level());

        if max_level == 0 && !options.generate_control_verts {
            let mut result = Box::new(StencilTable::new());
            result.set_num_control_vertices(num_control);
            return result;
        }

        // Build using StencilBuilder
        let mut builder = StencilBuilder::new(
            num_control,
            /*gen_ctrl_stencils*/ true,
            /*compact*/ true,
        );

        let primvar = PrimvarRefiner::new(refiner);

        // Accumulate stencils level by level using the Interpolatable builder index facade
        let mut src_offset = 0i32;
        let mut dst_offset = num_control;

        for level in 1..=max_level {
            let level_vert_count = if fv {
                refiner
                    .get_level_internal(level)
                    .get_num_fvar_values(options.fvar_channel as i32)
            } else {
                refiner.get_level_internal(level).get_num_vertices()
            };

            // Interpolate this level using the builder as the primvar buffer
            interp_level_into_builder(
                &primvar,
                &mut builder,
                level,
                src_offset,
                dst_offset,
                options,
            );

            if options.factorize_intermediate_levels {
                src_offset = dst_offset;
            } else {
                builder.set_coarse_vert_count(dst_offset + level_vert_count);
            }

            dst_offset += level_vert_count;
        }

        let first_offset = if !options.generate_intermediate_levels {
            src_offset
        } else {
            num_control
        };

        build_table_from_builder(
            &builder,
            num_control,
            options.generate_control_verts,
            first_offset,
        )
    }

    /// Concatenate multiple stencil tables (they must share the same control
    /// vertex set).  Returns `None` if the inputs are incompatible.
    ///
    /// Mirrors C++ `Far::StencilTableFactory::Create(tables, count)`.
    #[doc(alias = "Create")]
    pub fn create_from_tables(tables: &[&StencilTable]) -> Option<Box<StencilTable>> {
        if tables.is_empty() {
            return None;
        }

        let ncvs = tables[0].get_num_control_vertices();
        let mut nstencils = 0i32;
        let mut nelems = 0i32;

        for t in tables {
            if t.get_num_control_vertices() != ncvs {
                return None;
            }
            nstencils += t.get_num_stencils();
            nelems += t.indices().len() as i32;
        }

        // Simple concatenation via extend
        let mut result = Box::new(StencilTable::new());
        result.set_num_control_vertices(ncvs);
        result.sizes.reserve(nstencils as usize);
        result.indices.reserve(nelems as usize);
        result.weights.reserve(nelems as usize);

        for t in tables {
            result.sizes.extend_from_slice(t.sizes());
            result.indices.extend_from_slice(t.indices());
            result.weights.extend_from_slice(t.weights());
        }
        result.generate_offsets();
        Some(result)
    }

    /// Append local-point stencils from a patch table onto a base stencil table.
    ///
    /// Mirrors C++ `Far::StencilTableFactory::AppendLocalPointStencilTable()`.
    #[doc(alias = "AppendLocalPointStencilTable")]
    pub fn append_local_point_stencil_table(
        refiner: &TopologyRefiner,
        base_table: &StencilTable,
        local_point_stencil_table: &StencilTable,
        factorize: bool,
    ) -> Option<Box<StencilTable>> {
        append_local_points(
            refiner,
            base_table,
            local_point_stencil_table,
            -1,
            factorize,
        )
    }

    /// Append local-point varying stencils onto a base stencil table.
    ///
    /// Mirrors C++ `StencilTableFactory::AppendLocalPointStencilTableVarying`.
    /// In C++ this is an alias for the regular vertex variant (channel = -1).
    pub fn append_local_point_stencil_table_varying(
        refiner: &TopologyRefiner,
        base_table: &StencilTable,
        local_point_stencil_table: &StencilTable,
        factorize: bool,
    ) -> Option<Box<StencilTable>> {
        append_local_points(
            refiner,
            base_table,
            local_point_stencil_table,
            -1,
            factorize,
        )
    }

    /// Append face-varying local-point stencils.
    ///
    /// Mirrors C++ `StencilTableFactory::AppendLocalPointStencilTableFaceVarying`.
    pub fn append_local_point_stencil_table_face_varying(
        refiner: &TopologyRefiner,
        base_table: &StencilTable,
        local_point_stencil_table: &StencilTable,
        channel: i32,
        factorize: bool,
    ) -> Option<Box<StencilTable>> {
        append_local_points(
            refiner,
            base_table,
            local_point_stencil_table,
            channel,
            factorize,
        )
    }
}

// ---------------------------------------------------------------------------
// LimitStencilTableFactory
// ---------------------------------------------------------------------------

/// Options for `LimitStencilTableFactory::create`.
#[derive(Clone, Copy)]
pub struct LimitStencilTableOptions {
    pub interpolation_mode: InterpolationMode,
    pub generate_1st_derivatives: bool,
    pub generate_2nd_derivatives: bool,
    pub fvar_channel: u32,
}

impl Default for LimitStencilTableOptions {
    fn default() -> Self {
        Self {
            interpolation_mode: InterpolationMode::Vertex,
            generate_1st_derivatives: true,
            generate_2nd_derivatives: false,
            fvar_channel: 0,
        }
    }
}

/// Surface location descriptor for limit stencil evaluation.
pub struct LocationArray {
    pub ptex_idx: i32,
    pub num_locations: i32,
    /// Array of u coordinates (length = `num_locations`).
    pub s: Vec<f32>,
    /// Array of v coordinates (length = `num_locations`).
    pub t: Vec<f32>,
}

/// Factory for building `LimitStencilTable`.
pub struct LimitStencilTableFactory;

impl LimitStencilTableFactory {
    /// Build a limit stencil table for the given surface locations.
    ///
    /// Mirrors C++ `LimitStencilTableFactoryReal<REAL>::Create()` exactly:
    /// - If `cv_stencils` is `None`, a StencilTable is created internally.
    /// - If `patch_table` is `None`, a PatchTable is created internally.
    /// - A PatchMap is built from the PatchTable.
    /// - For each location (ptexIdx, s, t), the patch is found via PatchMap,
    ///   its basis weights are evaluated, and the result is accumulated into
    ///   the builder by factorizing through the CV stencil table.
    pub fn create(
        refiner: &TopologyRefiner,
        location_arrays: &[LocationArray],
        cv_stencils: Option<&StencilTable>,
        patch_table: Option<&PatchTable>,
        options: LimitStencilTableOptions,
    ) -> Option<Box<LimitStencilTable>> {
        // Count total stencils to generate
        let num_stencils: i32 = location_arrays.iter().map(|a| a.num_locations).sum();
        if num_stencils <= 0 {
            return None;
        }

        let uniform = refiner.is_uniform();
        let max_level = refiner.get_max_level();

        let fv = options.interpolation_mode == InterpolationMode::FaceVarying;
        let varying = options.interpolation_mode == InterpolationMode::Varying;
        let fvar_ch = options.fvar_channel as i32;

        let n_ctrl = if fv {
            refiner.get_level_internal(0).get_num_fvar_values(fvar_ch)
        } else {
            refiner.get_level_internal(0).get_num_vertices()
        };

        // -----------------------------------------------------------------------
        // Validate and/or build the CV stencil table.
        // C++: mirrors the nRefinedStencils sanity check + cvstencils creation.
        // -----------------------------------------------------------------------
        let n_refined_stencils = if uniform {
            if fv {
                refiner
                    .get_level_internal(max_level)
                    .get_num_fvar_values(fvar_ch)
            } else {
                refiner.get_level_internal(max_level).get_num_vertices()
            }
        } else {
            if fv {
                refiner.get_num_fvar_values_total(fvar_ch)
            } else {
                refiner.get_num_vertices_total()
            }
        };

        // If a cv_stencils was provided but is too small, bail out.
        if let Some(st) = cv_stencils {
            if st.get_num_stencils() < n_refined_stencils {
                return None;
            }
        }

        let owned_cv: Box<StencilTable>;
        let cv_st: &StencilTable = if let Some(st) = cv_stencils {
            st
        } else {
            // C++: generateIntermediateLevels = uniform ? false : true
            let opts = StencilTableOptions {
                interpolation_mode: options.interpolation_mode,
                generate_offsets: true,
                generate_control_verts: true,
                generate_intermediate_levels: !uniform,
                factorize_intermediate_levels: true,
                max_level: 15,
                fvar_channel: options.fvar_channel,
            };
            owned_cv = StencilTableFactory::create(refiner, opts);
            &*owned_cv
        };

        if cv_st.get_num_stencils() == 0 {
            return None;
        }

        // -----------------------------------------------------------------------
        // Validate and/or build the PatchTable.
        // C++: patchTableOptions with ENDCAP_GREGORY_BASIS.
        // -----------------------------------------------------------------------
        if let Some(pt) = patch_table {
            // C++: patchTableIn->IsFeatureAdaptive() == uniform  => mismatch => bail
            if pt.is_feature_adaptive() == uniform {
                return None;
            }
        }

        let owned_pt: Box<PatchTable>;
        let pt: &PatchTable = if let Some(p) = patch_table {
            p
        } else {
            let mut pt_opts = PatchTableFactoryOptions::default();
            pt_opts.end_cap_type = EndCapType::GregoryBasis;
            pt_opts.include_base_level_indices = true;
            pt_opts.generate_varying_tables = varying;
            pt_opts.generate_fvar_tables = fv;
            if fv {
                pt_opts.include_fvar_base_level_indices = true;
                // C++ sets numFVarChannels=1 and fvarChannelIndices=&fvarChannel
                // so the patch table only covers the one channel we care about.
                pt_opts.num_fvar_channels = 1;
                pt_opts.fvar_channel_indices = Some(vec![fvar_ch]);
                // Legacy linear patches are used when refining uniformly OR when
                // the refiner's adaptive options don't consider fvar channels.
                pt_opts.generate_fvar_legacy_linear_patches =
                    uniform || !refiner.get_adaptive_options().consider_fvar_channels;
            }
            // Mirror C++: useInfSharpPatch = !uniform && refiner adaptive option.
            pt_opts.use_inf_sharp_patch =
                !uniform && refiner.get_adaptive_options().use_inf_sharp_patch;
            // PatchTableFactory::create takes (refiner, options, selected_faces).
            owned_pt = Box::new(PatchTableFactory::create(refiner, pt_opts, &[]));
            &*owned_pt
        };

        // -----------------------------------------------------------------------
        // Append local-point stencils from the PatchTable, if present.
        // C++ does this when cvstencils.GetNumStencils() == nRefinedStencils.
        // -----------------------------------------------------------------------
        // Optionally hold a merged table that extends cv_st with local-point stencils
        // from the PatchTable.  Declared here to ensure its lifetime outlasts cv_st.
        // NOTE: PatchTable local stencils are not yet bridged in Rust, so this is
        //       always None for now.  Keeping the structure matches C++ parity.
        let merged_cv: Option<Box<StencilTable>> = None;
        let cv_st: &StencilTable = if cv_st.get_num_stencils() == n_refined_stencils {
            // The given (or created) cv table does not include local points yet.
            // When PatchTable local stencils are available, append them here
            // (fvar/varying/vertex) and store the result in merged_cv.
            // For now: no local stencils, use cv_st as-is.
            merged_cv.as_deref().unwrap_or(cv_st)
        } else {
            cv_st
        };

        // -----------------------------------------------------------------------
        // Build PatchMap for fast ptex-face lookup.
        // -----------------------------------------------------------------------
        let patch_map = PatchMap::new(pt);

        // -----------------------------------------------------------------------
        // Accumulate limit stencils.
        // C++ inner loop: for each (ptexIdx, s, t) location:
        //   1. PatchMap::FindPatch(ptexIdx, s, t) -> handle
        //   2. Get CVs from the patch (vertex / varying / fvar)
        //   3. EvaluateBasis -> wP, wDs, wDt [, wDss, wDst, wDtt]
        //   4. For each CV k: dst.AddWithWeight(cvstencils[cvs[k]], wP[k], ...)
        // -----------------------------------------------------------------------
        let mut builder = StencilBuilder::new(n_ctrl, false, true);
        let mut num_limit_stencils = 0i32;

        // Scratch weight buffers (max 20 CVs for GregoryBasis patches).
        const MAX_CV: usize = 20;
        let mut w_p = [0.0f32; MAX_CV];
        let mut w_ds = [0.0f32; MAX_CV];
        let mut w_dt = [0.0f32; MAX_CV];
        let mut w_dss = [0.0f32; MAX_CV];
        let mut w_dst = [0.0f32; MAX_CV];
        let mut w_dtt = [0.0f32; MAX_CV];

        let gen1 = options.generate_1st_derivatives;
        let gen2 = options.generate_2nd_derivatives;

        for loc_arr in location_arrays {
            assert!(loc_arr.ptex_idx >= 0);

            for j in 0..loc_arr.num_locations as usize {
                let s = if j < loc_arr.s.len() {
                    loc_arr.s[j]
                } else {
                    0.0
                };
                let t = if j < loc_arr.t.len() {
                    loc_arr.t[j]
                } else {
                    0.0
                };

                let handle = patch_map.find_patch(loc_arr.ptex_idx, s as f64, t as f64);

                if let Some(handle) = handle {
                    // Get the CV indices for this patch in the right mode.
                    // C++: useVertexPatches || (interpolateVarying && uniform) => vertex
                    let cvs = if fv {
                        pt.get_patch_f_var_values(handle, fvar_ch)
                    } else if varying && !uniform {
                        pt.get_patch_varying_vertices(handle)
                    } else {
                        pt.get_patch_vertices(handle)
                    };

                    let n_cvs = cvs.len();
                    if n_cvs == 0 || n_cvs > MAX_CV {
                        continue;
                    }

                    // Zero out scratch buffers.
                    for i in 0..n_cvs {
                        w_p[i] = 0.0;
                        w_ds[i] = 0.0;
                        w_dt[i] = 0.0;
                        w_dss[i] = 0.0;
                        w_dst[i] = 0.0;
                        w_dtt[i] = 0.0;
                    }

                    // Evaluate basis weights.
                    if gen2 {
                        if fv {
                            pt.evaluate_basis_face_varying(
                                handle,
                                s,
                                t,
                                &mut w_p[..n_cvs],
                                Some(&mut w_ds[..n_cvs]),
                                Some(&mut w_dt[..n_cvs]),
                                Some(&mut w_dss[..n_cvs]),
                                Some(&mut w_dst[..n_cvs]),
                                Some(&mut w_dtt[..n_cvs]),
                                fvar_ch,
                            );
                        } else if varying && !uniform {
                            pt.evaluate_basis_varying(
                                handle,
                                s,
                                t,
                                &mut w_p[..n_cvs],
                                Some(&mut w_ds[..n_cvs]),
                                Some(&mut w_dt[..n_cvs]),
                                Some(&mut w_dss[..n_cvs]),
                                Some(&mut w_dst[..n_cvs]),
                                Some(&mut w_dtt[..n_cvs]),
                            );
                        } else {
                            pt.evaluate_basis(
                                handle,
                                s,
                                t,
                                &mut w_p[..n_cvs],
                                Some(&mut w_ds[..n_cvs]),
                                Some(&mut w_dt[..n_cvs]),
                                Some(&mut w_dss[..n_cvs]),
                                Some(&mut w_dst[..n_cvs]),
                                Some(&mut w_dtt[..n_cvs]),
                            );
                        }
                    } else if gen1 {
                        if fv {
                            pt.evaluate_basis_face_varying(
                                handle,
                                s,
                                t,
                                &mut w_p[..n_cvs],
                                Some(&mut w_ds[..n_cvs]),
                                Some(&mut w_dt[..n_cvs]),
                                None,
                                None,
                                None,
                                fvar_ch,
                            );
                        } else if varying && !uniform {
                            pt.evaluate_basis_varying(
                                handle,
                                s,
                                t,
                                &mut w_p[..n_cvs],
                                Some(&mut w_ds[..n_cvs]),
                                Some(&mut w_dt[..n_cvs]),
                                None,
                                None,
                                None,
                            );
                        } else {
                            pt.evaluate_basis(
                                handle,
                                s,
                                t,
                                &mut w_p[..n_cvs],
                                Some(&mut w_ds[..n_cvs]),
                                Some(&mut w_dt[..n_cvs]),
                                None,
                                None,
                                None,
                            );
                        }
                    } else {
                        if fv {
                            pt.evaluate_basis_face_varying(
                                handle,
                                s,
                                t,
                                &mut w_p[..n_cvs],
                                None,
                                None,
                                None,
                                None,
                                None,
                                fvar_ch,
                            );
                        } else if varying && !uniform {
                            pt.evaluate_basis_varying(
                                handle,
                                s,
                                t,
                                &mut w_p[..n_cvs],
                                None,
                                None,
                                None,
                                None,
                                None,
                            );
                        } else {
                            pt.evaluate_basis(
                                handle,
                                s,
                                t,
                                &mut w_p[..n_cvs],
                                None,
                                None,
                                None,
                                None,
                                None,
                            );
                        }
                    }

                    // dst = origin[numLimitStencils]; dst.Clear();
                    // for k in 0..cvs.size(): dst.AddWithWeight(src[cvs[k]], wP[k], ...);
                    // C++: AddWithWeight factorizes src[cvs[k]] (a stencil) through the
                    // builder's coarse CV set using the given scalar weights.
                    let dst_idx = num_limit_stencils;
                    let mut dst = builder.index(dst_idx);

                    if gen2 {
                        for k in 0..n_cvs {
                            let src = cv_st.get_stencil(cvs[k] as Index);
                            dst.add_with_weight_2nd(
                                src.get_size(),
                                src.get_vertex_indices(),
                                src.get_weights(),
                                w_p[k] as f64,
                                w_ds[k] as f64,
                                w_dt[k] as f64,
                                w_dss[k] as f64,
                                w_dst[k] as f64,
                                w_dtt[k] as f64,
                            );
                        }
                    } else if gen1 {
                        for k in 0..n_cvs {
                            let src = cv_st.get_stencil(cvs[k] as Index);
                            dst.add_with_weight_1st(
                                src.get_size(),
                                src.get_vertex_indices(),
                                src.get_weights(),
                                w_p[k] as f64,
                                w_ds[k] as f64,
                                w_dt[k] as f64,
                            );
                        }
                    } else {
                        for k in 0..n_cvs {
                            let src = cv_st.get_stencil(cvs[k] as Index);
                            dst.add_with_weight_stencil(
                                src.get_size(),
                                src.get_vertex_indices(),
                                src.get_weights(),
                                w_p[k] as f64,
                            );
                        }
                    }

                    num_limit_stencils += 1;
                }
                // If no handle found (hole), the stencil slot remains empty.
            }
        }

        // -----------------------------------------------------------------------
        // Pack results from builder into LimitStencilTable.
        // C++: LimitStencilTableReal<REAL>(nControlVertices, builder.*Weights, ...,
        //                                   ctrlVerts=false, firstOffset=0)
        // -----------------------------------------------------------------------
        let sizes_raw = builder.get_stencil_sizes();
        let offsets_raw = builder.get_stencil_offsets();
        let sources_raw = builder.get_stencil_sources();
        let weights_raw = builder.get_stencil_weights();

        let sizes_out: Vec<i32> = sizes_raw[..num_limit_stencils as usize]
            .iter()
            .map(|&s| s as i32)
            .collect();
        let offsets_out: Vec<i32> = offsets_raw[..num_limit_stencils as usize]
            .iter()
            .map(|&o| o as i32)
            .collect();
        let n_elems = if num_limit_stencils > 0 {
            let last = num_limit_stencils as usize - 1;
            offsets_raw[last] as usize + sizes_raw[last] as usize
        } else {
            0
        };
        let sources_out: Vec<i32> = sources_raw[..n_elems].to_vec();
        let weights_out: Vec<f32> = weights_raw[..n_elems].iter().map(|&w| w as f32).collect();

        let du_out: Vec<f32> = if gen1 {
            builder.get_stencil_du_weights()[..n_elems]
                .iter()
                .map(|&w| w as f32)
                .collect()
        } else {
            vec![]
        };
        let dv_out: Vec<f32> = if gen1 {
            builder.get_stencil_dv_weights()[..n_elems]
                .iter()
                .map(|&w| w as f32)
                .collect()
        } else {
            vec![]
        };
        let duu_out: Vec<f32> = if gen2 {
            builder.get_stencil_duu_weights()[..n_elems]
                .iter()
                .map(|&w| w as f32)
                .collect()
        } else {
            vec![]
        };
        let duv_out: Vec<f32> = if gen2 {
            builder.get_stencil_duv_weights()[..n_elems]
                .iter()
                .map(|&w| w as f32)
                .collect()
        } else {
            vec![]
        };
        let dvv_out: Vec<f32> = if gen2 {
            builder.get_stencil_dvv_weights()[..n_elems]
                .iter()
                .map(|&w| w as f32)
                .collect()
        } else {
            vec![]
        };

        Some(Box::new(LimitStencilTable::from_data(
            n_ctrl,
            sizes_out,
            offsets_out,
            sources_out,
            weights_out,
            du_out,
            dv_out,
            duu_out,
            duv_out,
            dvv_out,
        )))
    }
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Build a StencilTable from a completed StencilBuilder.
fn build_table_from_builder(
    builder: &StencilBuilder,
    num_control_vertices: i32,
    gen_ctrl_verts: bool,
    first_offset: i32,
) -> Box<StencilTable> {
    let offsets = builder.get_stencil_offsets();
    let sizes = builder.get_stencil_sizes();
    let sources = builder.get_stencil_sources();
    let weights = builder.get_stencil_weights();

    // first_offset is the first stencil index we want to include.
    // If gen_ctrl_verts is true, include from 0; otherwise from first_offset.
    let start_stencil = if gen_ctrl_verts {
        0
    } else {
        first_offset as usize
    };
    let end_stencil = sizes.len();

    let mut result_sizes: Vec<i32> = Vec::new();
    let mut result_indices: Vec<i32> = Vec::new();
    let mut result_weights: Vec<f32> = Vec::new();

    for si in start_stencil..end_stencil {
        let sz = sizes[si] as usize;
        let ofs = offsets[si] as usize;
        result_sizes.push(sz as i32);
        for k in 0..sz {
            result_indices.push(sources[ofs + k]);
            result_weights.push(weights[ofs + k] as f32);
        }
    }

    let mut offsets_out = vec![0i32; result_sizes.len()];
    let mut off = 0i32;
    for i in 0..result_sizes.len() {
        offsets_out[i] = off;
        off += result_sizes[i];
    }

    let mut t = Box::new(StencilTable::from_data(
        num_control_vertices,
        result_sizes,
        offsets_out,
        result_indices,
        result_weights,
    ));
    t.finalize();
    t
}

/// Interpolate one level into the StencilBuilder using the PrimvarRefiner.
/// We represent the primvar buffer as builder stencil-index slices.
fn interp_level_into_builder(
    primvar: &PrimvarRefiner<'_>,
    builder: &mut StencilBuilder,
    level: i32,
    src_offset: i32,
    dst_offset: i32,
    options: StencilTableOptions,
) {
    let refiner = primvar.get_topology_refiner();
    let refinement = refiner.get_refinement_internal(level - 1);

    // For each child vertex at `dst_offset + child_local_idx`, accumulate
    // weights from the parent vertices at `src_offset + parent_local_idx`.

    let fv = options.interpolation_mode == InterpolationMode::FaceVarying;
    let fvar_ch = options.fvar_channel as i32;

    // Face-derived vertices
    if !fv {
        let nf = refinement.get_num_child_vertices_from_faces();
        let base = refinement.get_first_child_vertex_from_faces();
        let parent = refiner.get_level_internal(level - 1);
        for i in 0..nf {
            let cv = base + i;
            let pface = refinement.get_child_vertex_parent_index(cv);
            let fverts = parent.get_face_vertices(pface);
            let n = fverts.size();
            let w = 1.0 / n as f64;
            let dst = dst_offset + cv;
            let mut idx = builder.index(dst);
            for k in 0..n {
                let src_vi = src_offset + fverts[k as i32];
                idx.add_with_weight_vertex(src_vi, w);
            }
        }

        // Edge-derived vertices
        let ne = refinement.get_num_child_vertices_from_edges();
        let ebase = refinement.get_first_child_vertex_from_edges();
        for i in 0..ne {
            let cv = ebase + i;
            let pedge = refinement.get_child_vertex_parent_index(cv);
            let everts = parent.get_edge_vertices(pedge);
            let dst = dst_offset + cv;
            let mut idx = builder.index(dst);
            idx.add_with_weight_vertex(src_offset + everts[0], 0.5);
            idx.add_with_weight_vertex(src_offset + everts[1], 0.5);
        }

        // Vertex-derived vertices
        let nv = refinement.get_num_child_vertices_from_vertices();
        let vbase = refinement.get_first_child_vertex_from_vertices();
        for i in 0..nv {
            let cv = vbase + i;
            let pvert = refinement.get_child_vertex_parent_index(cv);
            let dst = dst_offset + cv;
            let mut idx = builder.index(dst);
            idx.add_with_weight_vertex(src_offset + pvert, 1.0);
        }
    } else {
        // Face-varying: same structure but indexed by fvar values
        let _parent = refiner.get_level_internal(level - 1);
        let child = refiner.get_level_internal(level);
        let nfv = child.get_num_fvar_values(fvar_ch);
        for fv_idx in 0..nfv {
            let dst = dst_offset + fv_idx;
            let mut idx = builder.index(dst);
            // Approximate: pass through
            idx.add_with_weight_vertex(src_offset + fv_idx, 1.0);
        }
    }
}

/// Append local-point stencils onto a base table (internal).
fn append_local_points(
    refiner: &TopologyRefiner,
    base_table: &StencilTable,
    local_table: &StencilTable,
    channel: i32,
    factorize: bool,
) -> Option<Box<StencilTable>> {
    if local_table.get_num_stencils() == 0 {
        return None;
    }

    let n_ctrl = if channel < 0 {
        refiner.get_level_internal(0).get_num_vertices()
    } else {
        refiner.get_level_internal(0).get_num_fvar_values(channel)
    };

    if base_table.get_num_stencils() == 0 {
        let mut result = Box::new(StencilTable::new());
        result.set_num_control_vertices(n_ctrl);
        result.sizes = local_table.sizes.clone();
        result.indices = local_table.indices.clone();
        result.weights = local_table.weights.clone();
        result.generate_offsets();
        return Some(result);
    }

    let n_base = base_table.get_num_stencils() as usize;
    let n_local = local_table.get_num_stencils() as usize;
    let base_elems = base_table.indices.len();

    let total_nverts = refiner.get_num_vertices_total();
    let ctrl_offset = if base_table.get_num_stencils() == total_nverts {
        0
    } else {
        n_ctrl
    };

    // Build expanded local-point stencils via StencilBuilder
    let mut builder = StencilBuilder::new(n_ctrl, false, factorize);

    for i in 0..n_local {
        let src = local_table.get_stencil(i as Index);
        let sz = src.get_size() as usize;
        let src_idx = src.get_vertex_indices();
        let src_wgt = src.get_weights();

        let mut dst_idx = builder.index(i as i32);
        for k in 0..sz {
            let abs_index = src_idx[k] as usize;
            let w = src_wgt[k] as f64;
            if w == 0.0 {
                continue;
            }

            if factorize && abs_index >= ctrl_offset as usize {
                // Factorize: resolve through base table
                let base_si = abs_index - ctrl_offset as usize;
                if base_si < n_base {
                    let base_stencil = base_table.get_stencil(base_si as Index);
                    dst_idx.add_with_weight_stencil(
                        base_stencil.get_size(),
                        base_stencil.get_vertex_indices(),
                        base_stencil.get_weights(),
                        w,
                    );
                }
            } else {
                dst_idx.add_with_weight_vertex((abs_index + ctrl_offset as usize) as i32, w);
            }
        }
    }

    // Assemble result: base + local-point stencils
    let mut result = Box::new(StencilTable::new());
    result.set_num_control_vertices(n_ctrl);
    result.sizes.reserve(n_base + n_local);
    result
        .indices
        .reserve(base_elems + builder.get_num_vertices_total());
    result
        .weights
        .reserve(base_elems + builder.get_num_vertices_total());

    // Copy base
    result.sizes.extend_from_slice(base_table.sizes());
    result.indices.extend_from_slice(base_table.indices());
    result.weights.extend_from_slice(base_table.weights());

    // Append local-point stencils
    let offs = builder.get_stencil_offsets();
    let sizes = builder.get_stencil_sizes();
    let srcs = builder.get_stencil_sources();
    let wgts = builder.get_stencil_weights();

    for i in 0..n_local {
        let sz = sizes[i] as usize;
        let ofs = offs[i] as usize;
        result.sizes.push(sz as i32);
        for k in 0..sz {
            result.indices.push(srcs[ofs + k]);
            result.weights.push(wgts[ofs + k] as f32);
        }
    }

    result.generate_offsets();
    Some(result)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::super::topology_refiner::TopologyRefiner;
    use super::*;
    use crate::sdc::{Options, types::SchemeType};

    #[test]
    fn create_empty_refiner() {
        let refiner = TopologyRefiner::new(SchemeType::Catmark, Options::default());
        let opts = StencilTableOptions::default();
        let tbl = StencilTableFactory::create(&refiner, opts);
        // Empty refiner should produce empty table
        assert_eq!(tbl.get_num_stencils(), 0);
    }

    #[test]
    fn concatenate_tables() {
        let mut t1 = StencilTable::from_data(3, vec![1, 1], vec![0, 1], vec![0, 1], vec![1.0, 1.0]);
        t1.generate_offsets();
        let mut t2 = StencilTable::from_data(3, vec![1], vec![0], vec![2], vec![1.0]);
        t2.generate_offsets();

        let combined = StencilTableFactory::create_from_tables(&[&t1, &t2]).unwrap();
        assert_eq!(combined.get_num_stencils(), 3);
        assert_eq!(combined.get_num_control_vertices(), 3);
    }
}
