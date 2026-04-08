//! SurfaceFactory — base class for initializing limit surfaces.
//!
//! Ported from OpenSubdiv bfr/surfaceFactory.h/.cpp.
//!
//! This is the abstract factory that:
//!   1. Gathers the topological neighbourhood of a face via the
//!      `SurfaceFactoryMeshAdapter` trait (implemented by subclasses).
//!   2. Determines whether the face has a limit surface.
//!   3. Initialises a `Surface<R>` as regular, linear, or irregular.
//!   4. Caches irregular patch trees via `SurfaceFactoryCache`.

use std::sync::{Arc, RwLock};

use crate::sdc::options::Options as SdcOptions;
use crate::sdc::types::{SchemeType, SchemeTypeTraits};

use super::face_surface::FaceSurface;
use super::face_topology::FaceTopology;
use super::hash;
use super::irregular_patch_builder::{IrregPatchOptions, IrregularPatchBuilder};
use super::limits::Limits;
use super::parameterization::Parameterization;
use super::regular_patch_builder::RegularPatchBuilder;
use super::surface::Surface;
use super::surface::SurfaceReal;
use super::surface_data::SurfaceData;
use super::surface_factory_cache::{CacheKey, SurfaceFactoryCache, SurfaceFactoryCacheTrait};
use super::surface_factory_mesh_adapter::{FVarId, Index, SurfaceFactoryMeshAdapter};
use super::vertex_descriptor::VertexDescriptor;

// ---------------------------------------------------------------------------
//  SurfaceFactory Options
// ---------------------------------------------------------------------------

/// Options that control the behaviour of a `SurfaceFactory` instance.
///
/// Mirrors `Bfr::SurfaceFactory::Options`.
#[derive(Clone)]
pub struct SurfaceFactoryOptions {
    /// Default face-varying channel ID (-1 = none).
    pub default_fvar_id: FVarId,
    /// Enable/disable the internal topology cache.
    pub caching_enabled: bool,
    /// Maximum refinement depth for smooth features.
    pub approx_level_smooth: u8,
    /// Maximum refinement depth for sharp features.
    pub approx_level_sharp: u8,
    /// Optional external cache shared across multiple factories.
    ///
    /// When set and caching is enabled, the factory uses this cache instead
    /// of its per-instance internal cache.  Mirrors C++
    /// `SurfaceFactory::Options::SetExternalCache`.
    pub external_cache: Option<Arc<dyn SurfaceFactoryCacheTrait>>,
}

impl std::fmt::Debug for SurfaceFactoryOptions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SurfaceFactoryOptions")
            .field("default_fvar_id", &self.default_fvar_id)
            .field("caching_enabled", &self.caching_enabled)
            .field("approx_level_smooth", &self.approx_level_smooth)
            .field("approx_level_sharp", &self.approx_level_sharp)
            .field("external_cache", &self.external_cache.is_some())
            .finish()
    }
}

impl Default for SurfaceFactoryOptions {
    fn default() -> Self {
        SurfaceFactoryOptions {
            default_fvar_id: -1,
            caching_enabled: true,
            approx_level_smooth: 2,
            approx_level_sharp: 6,
            external_cache: None,
        }
    }
}

// ---------------------------------------------------------------------------
//  SurfaceFactory
// ---------------------------------------------------------------------------

/// Abstract base factory.  Subclasses implement `SurfaceFactoryMeshAdapter`
/// and call `new_factory()` in their own constructors.
///
/// Mirrors `Bfr::SurfaceFactory`.
pub struct SurfaceFactory {
    // Subdivision configuration:
    subdiv_scheme: SchemeType,
    subdiv_options: SdcOptions,

    // Derived flags:
    linear_scheme: bool,
    linear_fvar_interp: bool,
    test_neighborhood_for_limit: bool,
    reject_smooth_boundaries_for_limit: bool,
    reject_irregular_faces_for_limit: bool,

    reg_face_size: i32,

    // Options:
    factory_options: SurfaceFactoryOptions,

    // Cache (may be the internal one or an external one):
    // internal_cache keeps the per-instance Arc alive even when topology_cache
    // points at an external shared cache.  Not read directly — kept for RAII.
    #[allow(dead_code)]
    internal_cache: Arc<RwLock<SurfaceFactoryCache>>,
    topology_cache: Option<Arc<dyn SurfaceFactoryCacheTrait>>,
}

impl SurfaceFactory {
    /// Construct the base factory state.
    ///
    /// Called by subclass constructors (typically via `SurfaceFactoryBase::new`
    /// which they embed as a member or call via their own `new`).
    pub fn new(
        scheme_type: SchemeType,
        scheme_options: SdcOptions,
        factory_opts: SurfaceFactoryOptions,
    ) -> Self {
        let reg_face_size = SchemeTypeTraits::regular_face_size(scheme_type) as i32;
        let local_neighborhood = SchemeTypeTraits::local_neighborhood_size(scheme_type);
        let linear_scheme = local_neighborhood == 0;

        use crate::sdc::options::{FVarLinearInterpolation, VtxBoundaryInterpolation};
        let linear_fvar_interp = linear_scheme
            || scheme_options.get_fvar_linear_interpolation() == FVarLinearInterpolation::All;

        let reject_smooth = !linear_scheme
            && scheme_options.get_vtx_boundary_interpolation() == VtxBoundaryInterpolation::None;

        let reject_irregular = !linear_scheme && (reg_face_size == 3);

        let test_nbhd = reject_smooth || reject_irregular;

        let internal_cache = Arc::new(RwLock::new(SurfaceFactoryCache::new()));
        // Use external cache if provided, otherwise fall back to the internal one.
        // Mirrors C++ SurfaceFactory::setFactoryOptions where _externCache takes
        // precedence over the per-instance internal cache.
        let topology_cache: Option<Arc<dyn SurfaceFactoryCacheTrait>> =
            if factory_opts.caching_enabled {
                if let Some(ref ext) = factory_opts.external_cache {
                    Some(ext.clone())
                } else {
                    Some(internal_cache.clone() as Arc<dyn SurfaceFactoryCacheTrait>)
                }
            } else {
                None
            };

        SurfaceFactory {
            subdiv_scheme: scheme_type,
            subdiv_options: scheme_options,
            linear_scheme,
            linear_fvar_interp,
            test_neighborhood_for_limit: test_nbhd,
            reject_smooth_boundaries_for_limit: reject_smooth,
            reject_irregular_faces_for_limit: reject_irregular,
            reg_face_size,
            factory_options: factory_opts,
            internal_cache,
            topology_cache,
        }
    }

    // -----------------------------------------------------------------------
    //  Subdivision queries
    // -----------------------------------------------------------------------

    pub fn get_scheme_type(&self) -> SchemeType {
        self.subdiv_scheme
    }
    pub fn get_scheme_options(&self) -> SdcOptions {
        self.subdiv_options
    }

    // -----------------------------------------------------------------------
    //  Cache management
    // -----------------------------------------------------------------------

    /// Replace the internal cache with `cache` (used by subclasses).
    pub fn set_internal_cache(&mut self, cache: Arc<dyn SurfaceFactoryCacheTrait>) {
        if self.factory_options.caching_enabled {
            self.topology_cache = Some(cache);
        }
    }

    // -----------------------------------------------------------------------
    //  Face-has-limit tests
    // -----------------------------------------------------------------------

    /// Return whether a face has an associated limit surface.
    pub fn face_has_limit_surface(
        &self,
        adapter: &dyn SurfaceFactoryMeshAdapter,
        face_index: Index,
    ) -> bool {
        let face_size = adapter.get_face_size(face_index);
        if !self.face_has_limit_simple(adapter, face_index, face_size) {
            return false;
        }
        if self.test_neighborhood_for_limit {
            // Quick regularity check (no full topology needed):
            if !self.is_face_neighborhood_regular(adapter, face_index, None, &mut []) {
                return self.face_has_limit_neighborhood(adapter, face_index);
            }
        }
        true
    }

    /// Return the parameterization of the face.
    pub fn get_face_parameterization(
        &self,
        adapter: &dyn SurfaceFactoryMeshAdapter,
        face_index: Index,
    ) -> Parameterization {
        Parameterization::new(self.subdiv_scheme, adapter.get_face_size(face_index))
    }

    // -----------------------------------------------------------------------
    //  Surface initialization (the main public API)
    // -----------------------------------------------------------------------

    /// Initialize `surface` for vertex-interpolated data on `face_index`.
    pub fn init_vertex_surface<R: SurfaceReal>(
        &self,
        adapter: &dyn SurfaceFactoryMeshAdapter,
        face_index: Index,
        surface: &mut Surface<R>,
    ) -> bool {
        self.init_surfaces_impl(
            adapter,
            face_index,
            Some(surface.get_surface_data_mut()),
            None,
            &[],
            &mut [],
        )
    }

    /// Initialize `surface` for varying-interpolated data on `face_index`.
    pub fn init_varying_surface<R: SurfaceReal>(
        &self,
        adapter: &dyn SurfaceFactoryMeshAdapter,
        face_index: Index,
        surface: &mut Surface<R>,
    ) -> bool {
        self.init_surfaces_impl(
            adapter,
            face_index,
            None,
            Some(surface.get_surface_data_mut()),
            &[],
            &mut [],
        )
    }

    /// Initialize a face-varying surface using the default fvar ID from options.
    ///
    /// Mirrors `SurfaceFactory::InitFaceVaryingSurface(faceIndex, surface)` (no fvarID arg).
    pub fn init_default_fvar_surface<R: SurfaceReal>(
        &self,
        adapter: &dyn SurfaceFactoryMeshAdapter,
        face_index: Index,
        surface: &mut Surface<R>,
    ) -> bool {
        let fvar_id = self.factory_options.default_fvar_id;
        if fvar_id < 0 {
            return false;
        }
        self.init_surfaces_impl(
            adapter,
            face_index,
            None,
            None,
            &[fvar_id],
            std::slice::from_mut(surface.get_surface_data_mut()),
        )
    }

    /// Initialize a face-varying surface for `fvar_id`.
    pub fn init_fvar_surface<R: SurfaceReal>(
        &self,
        adapter: &dyn SurfaceFactoryMeshAdapter,
        face_index: Index,
        fvar_id: FVarId,
        surface: &mut Surface<R>,
    ) -> bool {
        self.init_surfaces_impl(
            adapter,
            face_index,
            None,
            None,
            &[fvar_id],
            std::slice::from_mut(surface.get_surface_data_mut()),
        )
    }

    /// Initialize vertex, varying, and face-varying surfaces in a single call.
    ///
    /// This is the batch form that avoids repeated topology construction.
    /// Any argument that is `None` / empty is skipped.  If `fvar_ids` is
    /// empty and `fvar_surfaces` is non-empty the default fvar ID from the
    /// factory options is used (same semantics as C++ `InitSurfaces`).
    ///
    /// Mirrors `Bfr::SurfaceFactory::InitSurfaces<REAL>`.
    pub fn init_surfaces<R: SurfaceReal>(
        &self,
        adapter: &dyn SurfaceFactoryMeshAdapter,
        face_index: Index,
        vtx_surface: Option<&mut Surface<R>>,
        var_surface: Option<&mut Surface<R>>,
        fvar_surfaces: &mut [Surface<R>],
        fvar_ids: &[FVarId],
    ) -> bool {
        // When fvar surfaces are requested but no IDs provided, use the
        // default fvar ID (mirrors C++ useDfltFVarID logic).
        let default_id = self.factory_options.default_fvar_id;
        let use_default = !fvar_surfaces.is_empty() && fvar_ids.is_empty();
        let effective_ids: &[FVarId] = if use_default {
            if default_id < 0 {
                return false;
            }
            std::slice::from_ref(&default_id)
        } else {
            fvar_ids
        };

        // Extract SurfaceData refs.  The Option<&mut Surface<R>> parameters
        // must be split into Option<&mut SurfaceData> for init_surfaces_impl.
        let vtx_data = vtx_surface.map(|s| &mut s.data);
        let var_data = var_surface.map(|s| &mut s.data);

        // For fvar surfaces, collect SurfaceData refs via slice.
        // Each entry in fvar_surfaces maps 1:1 to fvar_data entries.
        let mut fvar_data: Vec<SurfaceData> =
            fvar_surfaces.iter().map(|s| s.data.clone()).collect();

        let result = self.init_surfaces_impl(
            adapter,
            face_index,
            vtx_data,
            var_data,
            effective_ids,
            &mut fvar_data,
        );

        // Write results back: fvar data was cloned in, now write back out.
        for (surf, data) in fvar_surfaces.iter_mut().zip(fvar_data.into_iter()) {
            surf.data = data;
        }
        result
    }

    // -----------------------------------------------------------------------
    //  Internal implementation
    // -----------------------------------------------------------------------

    fn face_has_limit_simple(
        &self,
        adapter: &dyn SurfaceFactoryMeshAdapter,
        face_index: Index,
        face_size: i32,
    ) -> bool {
        face_size >= 3 && face_size <= Limits::max_face_size() && !adapter.is_face_hole(face_index)
    }

    fn face_has_limit_neighborhood(
        &self,
        adapter: &dyn SurfaceFactoryMeshAdapter,
        face_index: Index,
    ) -> bool {
        let face_size = adapter.get_face_size(face_index) as usize;
        let mut vtx_desc;
        let mut idx_buf = vec![0i32; 128];

        for i in 0..face_size {
            vtx_desc = VertexDescriptor::new();
            let face_in_ring =
                adapter.populate_face_vertex_descriptor(face_index, i as i32, &mut vtx_desc);
            if face_in_ring < 0 {
                return false;
            }

            use super::face_vertex::FaceVertex;
            let mut fv = FaceVertex::new();
            fv.initialize(face_size as i32, self.reg_face_size);
            // Copy descriptor:
            *fv.get_vertex_descriptor_mut() = vtx_desc.clone();
            fv.finalize(face_in_ring);

            let tag = fv.get_tag();
            if self.reject_smooth_boundaries_for_limit {
                if tag.is_un_ordered() {
                    let nfv = fv.get_num_face_vertices() as usize;
                    idx_buf.resize(nfv, 0);
                    if adapter.get_face_vertex_incident_face_vertex_indices(
                        face_index,
                        i as i32,
                        &mut idx_buf[..nfv],
                    ) < 0
                    {
                        return false;
                    }
                    fv.connect_un_ordered_faces(&idx_buf[..nfv]);
                }
                if fv.get_tag().has_non_sharp_boundary() {
                    return false;
                }
            }
            if self.reject_irregular_faces_for_limit {
                if tag.has_irregular_face_sizes() {
                    return false;
                }
            }
        }
        true
    }

    /// Returns true when the face's neighbourhood is a simple regular patch
    /// and optionally fills `indices` with the patch control-vertex indices.
    fn is_face_neighborhood_regular(
        &self,
        adapter: &dyn SurfaceFactoryMeshAdapter,
        face_index: Index,
        fvar_ptr: Option<FVarId>,
        indices: &mut [Index],
    ) -> bool {
        if let Some(fvar_id) = fvar_ptr {
            adapter.get_face_neighborhood_fvar_value_indices_if_regular(
                face_index,
                fvar_id,
                if indices.is_empty() {
                    None
                } else {
                    Some(indices)
                },
            )
        } else {
            adapter.get_face_neighborhood_vertex_indices_if_regular(
                face_index,
                if indices.is_empty() {
                    None
                } else {
                    Some(indices)
                },
            )
        }
    }

    // -----------------------------------------------------------------------
    //  Core: init_surfaces_impl
    // -----------------------------------------------------------------------

    fn init_surfaces_impl(
        &self,
        adapter: &dyn SurfaceFactoryMeshAdapter,
        face_index: Index,
        mut vtx_surf: Option<&mut SurfaceData>,
        mut var_surf: Option<&mut SurfaceData>,
        fvar_ids: &[FVarId],
        fvar_surfs: &mut [SurfaceData], // length == fvar_ids.len()
    ) -> bool {
        let face_size = adapter.get_face_size(face_index);

        // Reinitialize all target surfaces before (re)populating them.
        // Mirrors C++ initSurfaces() which calls reinitialize() on each.
        if let Some(s) = vtx_surf.as_mut() {
            s.reinitialize();
        }
        if let Some(s) = var_surf.as_mut() {
            s.reinitialize();
        }
        for s in fvar_surfs.iter_mut() {
            s.reinitialize();
        }

        if !self.face_has_limit_simple(adapter, face_index, face_size) {
            return false;
        }

        // Linear scheme: bilinear patches for everything.
        if self.linear_scheme {
            self.populate_linear_surfaces(
                adapter, face_index, vtx_surf, var_surf, fvar_ids, fvar_surfs,
            );
            return true;
        }

        // Try the fast regular path first.
        // For vertex / varying the fast path is checking regularity directly.
        let have_vertex_or_varying = vtx_surf.is_some() || var_surf.is_some();
        let have_fvar = !fvar_ids.is_empty();

        if have_vertex_or_varying && !have_fvar {
            // Allocate scratch index buffer for a regular patch.
            let patch_size = RegularPatchBuilder::patch_size_for(self.reg_face_size) as usize;
            let mut reg_indices = vec![0i32; patch_size];

            if self.is_face_neighborhood_regular(adapter, face_index, None, &mut reg_indices) {
                if let Some(surf) = vtx_surf {
                    self.assign_regular_surface_from_indices(surf, &reg_indices, face_size);
                }
                return true;
            }
        }

        // Non-regular (or fvar) path: build full FaceTopology.
        self.populate_non_linear_surfaces(
            adapter, face_index, vtx_surf, var_surf, fvar_ids, fvar_surfs,
        )
    }

    // -----------------------------------------------------------------------
    //  Assign a linear (bilinear) surface
    // -----------------------------------------------------------------------

    fn populate_linear_surfaces(
        &self,
        adapter: &dyn SurfaceFactoryMeshAdapter,
        face_index: Index,
        vtx_surf: Option<&mut SurfaceData>,
        var_surf: Option<&mut SurfaceData>,
        fvar_ids: &[FVarId],
        fvar_surfs: &mut [SurfaceData],
    ) {
        let face_size = adapter.get_face_size(face_index) as usize;
        let mut indices = vec![0i32; face_size];
        adapter.get_face_vertex_indices(face_index, &mut indices);

        let param = Parameterization::new(self.subdiv_scheme, face_size as i32);

        // OSD patch type for linear surfaces: QUADS=3 for quad meshes, TRIANGLES=4 for tri.
        // Mirrors C++ surfaceFactory.cpp assignLinearSurface() lines 682-684.
        let linear_ptype: u8 = if self.reg_face_size == 4 { 3 } else { 4 };

        let assign = |surf: &mut SurfaceData| {
            surf.resize_cvs(face_size);
            surf.get_cv_indices_mut().copy_from_slice(&indices);
            surf.set_param(param);
            surf.set_regular(false);
            surf.set_linear(true);
            surf.set_reg_patch_type(linear_ptype);
            surf.set_valid(true);
        };

        if let Some(s) = vtx_surf {
            assign(s);
        }
        if let Some(s) = var_surf {
            assign(s);
        }

        for (i, &fvar_id) in fvar_ids.iter().enumerate() {
            let mut fvar_indices = vec![0i32; face_size];
            adapter.get_face_fvar_value_indices(face_index, fvar_id, &mut fvar_indices);
            let s = &mut fvar_surfs[i];
            s.resize_cvs(face_size);
            s.get_cv_indices_mut().copy_from_slice(&fvar_indices);
            s.set_param(param);
            s.set_regular(false);
            s.set_linear(true);
            s.set_reg_patch_type(linear_ptype);
            s.set_valid(true);
        }
    }

    // -----------------------------------------------------------------------
    //  Assign a regular surface from pre-gathered patch-point indices
    // -----------------------------------------------------------------------

    fn assign_regular_surface_from_indices(
        &self,
        surf: &mut SurfaceData,
        indices: &[Index],
        face_size: i32,
    ) {
        let n = indices.len();
        surf.resize_cvs(n);
        surf.get_cv_indices_mut().copy_from_slice(indices);
        surf.set_param(Parameterization::new(self.subdiv_scheme, face_size));
        surf.set_regular(true);
        surf.set_linear(false);

        // Compute boundary mask from the gathered indices (-1 marks phantom CVs).
        let mask = RegularPatchBuilder::boundary_mask_from_cvs(self.reg_face_size, indices);
        surf.set_reg_patch_mask(mask as u8);

        // Regular patch type byte: OSD codes — REGULAR=6 (B-spline quad), LOOP=5 (box-spline tri).
        let ptype = if self.reg_face_size == 4 { 6u8 } else { 5u8 };
        surf.set_reg_patch_type(ptype);
        surf.set_valid(true);
    }

    // -----------------------------------------------------------------------
    //  Non-linear (irregular) path
    // -----------------------------------------------------------------------

    fn populate_non_linear_surfaces(
        &self,
        adapter: &dyn SurfaceFactoryMeshAdapter,
        face_index: Index,
        vtx_surf: Option<&mut SurfaceData>,
        _var_surf: Option<&mut SurfaceData>,
        fvar_ids: &[FVarId],
        fvar_surfs: &mut [SurfaceData],
    ) -> bool {
        let _face_size = adapter.get_face_size(face_index);

        // Build full topology.
        let mut topology = FaceTopology::new(self.subdiv_scheme, self.subdiv_options);
        if !self.gather_face_neighborhood_topology(adapter, face_index, &mut topology) {
            return false;
        }

        // Limit check (for Loop or VTX_BOUNDARY_NONE):
        if self.test_neighborhood_for_limit {
            if !self.face_has_limit_from_topology(&topology) {
                return false;
            }
        }

        // Gather vertex indices for this face's neighbourhood.
        let max_indices = topology.get_num_face_vertices() as usize;
        let mut vtx_indices = vec![0i32; max_indices];
        self.gather_face_neighborhood_indices(
            adapter,
            face_index,
            &topology,
            None,
            &mut vtx_indices,
        );

        // Build the vertex FaceSurface.
        let vtx_face_surface = FaceSurface::from_vertex(&topology, &vtx_indices);

        if vtx_face_surface.is_regular() {
            // Regular patch from FaceSurface.
            if let Some(surf) = vtx_surf {
                self.assign_regular_surface_from_face_surface(surf, &vtx_face_surface);
            }
        } else {
            // Irregular patch.
            if let Some(surf) = vtx_surf {
                self.assign_irregular_surface(surf, &vtx_face_surface);
            }
        }

        // FVar surfaces:
        for (i, &fvar_id) in fvar_ids.iter().enumerate() {
            let mut fvar_idx = vec![0i32; max_indices];
            self.gather_face_neighborhood_indices(
                adapter,
                face_index,
                &topology,
                Some(fvar_id),
                &mut fvar_idx,
            );
            let fvar_surface = FaceSurface::from_fvar(&vtx_face_surface, &fvar_idx);
            if fvar_surface.is_regular() {
                self.assign_regular_surface_from_face_surface(&mut fvar_surfs[i], &fvar_surface);
            } else if self.linear_fvar_interp {
                self.assign_linear_surface_from_face(&mut fvar_surfs[i], &fvar_surface);
            } else {
                self.assign_irregular_surface(&mut fvar_surfs[i], &fvar_surface);
            }
        }

        true
    }

    // -----------------------------------------------------------------------
    //  Face topology gathering
    // -----------------------------------------------------------------------

    fn gather_face_neighborhood_topology(
        &self,
        adapter: &dyn SurfaceFactoryMeshAdapter,
        face_index: Index,
        topology: &mut FaceTopology,
    ) -> bool {
        let face_size = adapter.get_face_size(face_index);
        topology.initialize(face_size);

        let mut idx_buf = vec![0i32; 256];

        for i in 0..face_size as usize {
            let corner = topology.get_topology_mut(i);
            corner.initialize(face_size, self.reg_face_size);

            let face_in_ring = adapter.populate_face_vertex_descriptor(
                face_index,
                i as i32,
                corner.get_vertex_descriptor_mut(),
            );
            if face_in_ring < 0 {
                return false;
            }

            corner.finalize(face_in_ring);
        }

        topology.finalize();

        if topology.has_un_ordered_corners() {
            // Gather all face-vertex indices so we can connect unordered corners.
            let nfv = topology.get_num_face_vertices() as usize;
            idx_buf.resize(nfv, 0);
            let mut offset = 0usize;
            for i in 0..face_size as usize {
                let corner = topology.get_topology(i);
                let n = corner.get_num_face_vertices() as usize;
                adapter.get_face_vertex_incident_face_vertex_indices(
                    face_index,
                    i as i32,
                    &mut idx_buf[offset..offset + n],
                );
                offset += n;
            }
            topology.resolve_un_ordered_corners(&idx_buf[..nfv]);
        }

        true
    }

    fn gather_face_neighborhood_indices(
        &self,
        adapter: &dyn SurfaceFactoryMeshAdapter,
        face_index: Index,
        topology: &FaceTopology,
        fvar_id: Option<FVarId>,
        indices: &mut [Index],
    ) {
        let face_size = topology.get_face_size() as usize;
        let mut offset = 0usize;

        for i in 0..face_size {
            let corner = topology.get_topology(i);
            let n = corner.get_num_face_vertices() as usize;
            let dst = &mut indices[offset..offset + n];

            if let Some(fv_id) = fvar_id {
                adapter.get_face_vertex_incident_face_fvar_value_indices(
                    face_index, i as i32, fv_id, dst,
                );
            } else {
                adapter.get_face_vertex_incident_face_vertex_indices(face_index, i as i32, dst);
            }
            offset += n;
        }
    }

    fn face_has_limit_from_topology(&self, topology: &FaceTopology) -> bool {
        let tag = topology.get_tag();
        if self.reject_smooth_boundaries_for_limit && tag.has_non_sharp_boundary() {
            return false;
        }
        if self.reject_irregular_faces_for_limit && tag.has_irregular_face_sizes() {
            return false;
        }
        true
    }

    // -----------------------------------------------------------------------
    //  Surface assignment helpers
    // -----------------------------------------------------------------------

    fn assign_regular_surface_from_face_surface(&self, surf: &mut SurfaceData, fs: &FaceSurface) {
        let builder = RegularPatchBuilder::new(fs);
        let patch_size = builder.get_num_control_vertices() as usize;
        surf.resize_cvs(patch_size);
        builder.gather_control_vertex_indices(surf.get_cv_indices_mut());
        surf.set_param(Parameterization::new(
            self.subdiv_scheme,
            fs.get_face_size(),
        ));
        surf.set_regular(true);
        surf.set_linear(false);
        let mask = builder.get_patch_param_boundary_mask();
        surf.set_reg_patch_mask(mask as u8);
        // OSD codes — REGULAR=6 (B-spline quad), LOOP=5 (box-spline tri).
        let ptype = if self.reg_face_size == 4 { 6u8 } else { 5u8 };
        surf.set_reg_patch_type(ptype);
        surf.set_valid(true);
    }

    fn assign_linear_surface_from_face(&self, surf: &mut SurfaceData, fs: &FaceSurface) {
        let face_size = fs.get_face_size() as usize;
        // The indices of the face vertices from the base-face portion.
        // In fvar_indices layout the base-face indices sit at the beginning
        // of the FaceTopology's flat index array.
        let c0 = fs.get_corner_topology(0);
        let base_off = c0.get_face_index_offset(c0.get_face()) as usize;
        let src = &fs.get_indices()[base_off..base_off + face_size];
        surf.resize_cvs(face_size);
        surf.get_cv_indices_mut().copy_from_slice(src);
        surf.set_param(Parameterization::new(self.subdiv_scheme, face_size as i32));
        surf.set_regular(false);
        surf.set_linear(true);
        surf.set_valid(true);
    }

    fn assign_irregular_surface(&self, surf: &mut SurfaceData, fs: &FaceSurface) {
        // Build a builder first — needed to check ControlHullDependsOnMeshIndices
        // before deciding whether the result can be cached.
        // This matches C++ order: builder first, then cache logic.
        let irreg_opts = IrregPatchOptions {
            sharp_level: self.factory_options.approx_level_sharp,
            smooth_level: self.factory_options.approx_level_smooth,
            double_precision: surf.is_double(),
        };
        let builder = IrregularPatchBuilder::new(fs, irreg_opts);

        // C++: if no cache OR control hull depends on mesh indices → skip cache.
        // Overlapping-face topology means the patch structure varies per-mesh-instance
        // and therefore cannot be reused from cache.
        let depends_on_indices = builder.control_hull_depends_on_mesh_indices();

        let irreg = if self.topology_cache.is_none() || depends_on_indices {
            // Build without caching.
            builder.build()
        } else {
            // Try to find an existing entry; build and insert if absent.
            let cache_key = self.compute_topology_key(fs);
            let cache = self.topology_cache.as_ref().unwrap();

            if let Some(cached) = cache.find(cache_key) {
                // Cache hit: reuse the patch tree, but still gather CV indices
                // from this mesh instance via the builder.
                let nc = cached.get_num_control_points() as usize;
                surf.resize_cvs(nc);
                builder.gather_control_vertex_indices(surf.get_cv_indices_mut());
                surf.set_param(Parameterization::new(
                    self.subdiv_scheme,
                    fs.get_face_size(),
                ));
                surf.set_regular(false);
                surf.set_linear(false);
                surf.set_irreg_patch_ptr(Some(cached));
                surf.set_valid(true);
                return;
            }

            // Cache miss: build and insert (use the instance the cache returns,
            // as another thread may have raced us).
            cache.add(cache_key, builder.build())
        };

        let nc = irreg.get_num_control_points() as usize;
        surf.resize_cvs(nc);
        builder.gather_control_vertex_indices(surf.get_cv_indices_mut());

        surf.set_param(Parameterization::new(
            self.subdiv_scheme,
            fs.get_face_size(),
        ));
        surf.set_regular(false);
        surf.set_linear(false);
        surf.set_irreg_patch_ptr(Some(irreg));
        surf.set_valid(true);
    }

    // -----------------------------------------------------------------------
    //  Cache key computation
    // -----------------------------------------------------------------------

    fn compute_topology_key(&self, fs: &FaceSurface) -> CacheKey {
        // Hash the topology of the FaceSurface into a 64-bit key.
        // Mirrors `hashTopologyKey` in surfaceFactory.cpp — must stay in sync.
        let mut buf: Vec<u8> = Vec::with_capacity(256);

        // Surface-level header: face size, scheme, approximation levels.
        let face_size = fs.get_face_size() as u16;
        buf.extend_from_slice(&face_size.to_le_bytes());
        buf.push(self.subdiv_scheme as u8);
        buf.push(self.factory_options.approx_level_sharp);
        buf.push(self.factory_options.approx_level_smooth);

        // Per-corner data — mirrors the CornerHeader struct in C++.
        for i in 0..face_size as usize {
            let c_top = fs.get_corner_topology(i);
            let c_sub = fs.get_corner_subset(i);

            let num_faces = c_sub.get_num_faces() as i16;
            let has_face_sz = c_sub.get_tag().has_un_common_face_sizes();
            let has_sh_edges = c_sub.get_tag().has_sharp_edges();
            let is_semi_sh = c_sub.get_tag().is_semi_sharp();

            buf.extend_from_slice(&num_faces.to_le_bytes());
            buf.extend_from_slice(&(c_sub.num_faces_before as i16).to_le_bytes());
            buf.push(c_sub.is_boundary() as u8);
            buf.push(c_sub.is_sharp() as u8);
            buf.push(is_semi_sh as u8);
            buf.push(has_face_sz as u8);
            buf.push(has_sh_edges as u8);

            // Semi-sharp vertex: use local_sharpness if set, otherwise vertex sharpness.
            if is_semi_sh {
                let s = if c_sub.local_sharpness > 0.0 {
                    c_sub.local_sharpness
                } else {
                    c_top.get_vertex_sharpness()
                };
                buf.extend_from_slice(&s.to_le_bytes());
            }

            // Variable face sizes within this corner's subset.
            if has_face_sz {
                let n = c_sub.get_num_faces() as usize;
                let mut f = c_top.get_face_first(c_sub);
                for _ in 0..n {
                    let sz = c_top.get_face_size(f) as i16;
                    buf.extend_from_slice(&sz.to_le_bytes());
                    f = c_top.get_face_next(f);
                }
            }

            // Sharp edge sharpness values (interior edges only).
            if has_sh_edges {
                let n_edges = (c_sub.get_num_faces() - c_sub.is_boundary() as i32) as usize;
                let mut f = c_top.get_face_first(c_sub);
                for _ in 0..n_edges {
                    let s = c_top.get_face_edge_sharpness(f, true);
                    buf.extend_from_slice(&s.to_le_bytes());
                    f = c_top.get_face_next(f);
                }
            }
        }

        hash::hash64(&buf)
    }
}

// ---------------------------------------------------------------------------
//  Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sdc::types::SchemeType;

    #[test]
    fn default_options() {
        let opts = SurfaceFactoryOptions::default();
        assert_eq!(opts.default_fvar_id, -1);
        assert!(opts.caching_enabled);
        assert_eq!(opts.approx_level_smooth, 2);
        assert_eq!(opts.approx_level_sharp, 6);
    }

    #[test]
    fn factory_scheme_type() {
        let f = SurfaceFactory::new(
            SchemeType::Catmark,
            SdcOptions::default(),
            SurfaceFactoryOptions::default(),
        );
        assert_eq!(f.get_scheme_type(), SchemeType::Catmark);
        assert_eq!(f.reg_face_size, 4);
        assert!(!f.linear_scheme);
    }

    #[test]
    fn bilinear_factory_is_linear() {
        let f = SurfaceFactory::new(
            SchemeType::Bilinear,
            SdcOptions::default(),
            SurfaceFactoryOptions::default(),
        );
        assert!(f.linear_scheme);
    }
}
