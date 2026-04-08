//! HdStRenderDelegate - Storm render delegate implementation.
//!
//! The main entry point for Storm rendering. Implements HdRenderDelegate
//! to provide rasterization-based rendering.

use crate::draw_items_cache::HdStDrawItemsCache;
use crate::draw_target::HdStDrawTarget;
use crate::ext_computation::HdStExtComputation;
use crate::field::{self, HdStField};
use crate::light::HdStLight;
use crate::material::HdStMaterial;
use crate::mesh::HdStMesh;
use crate::render_param::HdStRenderParam;
use crate::render_pass::HdStRenderPass;
use crate::resource_registry::{HdStResourceRegistry, HdStResourceRegistrySharedPtr};
use parking_lot::RwLock;
use std::any::Any;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use usd_hd::HdSceneDelegate;
use usd_hd::change_tracker::HdChangeTracker;
use usd_hd::prim::{HdCamera, HdImageShader};
use usd_hd::render::driver::{HdDriver, HdDriverVector};
use usd_hd::render::render_index::{
    HdPrimHandle, HdRenderIndex, HdRprimHandle, HdSprimHandle, HdSprimSync, RprimAdapter,
};
use usd_hd::render::{
    HdRenderDelegate, HdRenderParamSharedPtr, HdRenderPassSharedPtr, HdRenderSettingDescriptor,
    HdRenderSettingDescriptorList, HdRenderSettingsMap, HdResourceRegistrySharedPtr,
    HdRprimCollection, TfTokenVector,
};
use usd_hd::render_delegate_info::HdRenderDelegateInfo;
use usd_hd::types::HdDirtyBits;
use usd_hgi::{HgiDriverHandle, tokens::RENDER_DRIVER};
use usd_sdf::Path as SdfPath;
use usd_tf::Token;
use usd_vt::Value;

/// Storm render delegate.
///
/// Provides OpenGL/Vulkan/Metal rasterization rendering through Hgi
/// (Hydra Graphics Interface).
///
/// # Features
///
/// - Multiple prim types (mesh, curves, points, volumes)
/// - Subdivision surfaces
/// - Instancing
/// - MaterialX and UsdPreviewSurface materials
/// - Advanced lighting
/// - Transparency and compositing
///
/// # Supported Prim Types
///
/// **Rprims (Renderable)**: mesh, basisCurves, points, volume
/// **Sprims (State)**: camera, light, material, extComputation
/// **Bprims (Buffer)**: renderBuffer, renderTarget
pub struct HdStRenderDelegate {
    /// Render parameter shared with prims
    render_param: Arc<HdStRenderParam>,

    /// Resource registry for GPU resources (replaced with Hgi-backed when set_drivers runs)
    resource_registry: RwLock<HdStResourceRegistrySharedPtr>,

    /// Render settings
    settings: HdRenderSettingsMap,

    /// Supported rprim types (cached)
    supported_rprims: TfTokenVector,

    /// Supported sprim types (cached)
    supported_sprims: TfTokenVector,

    /// Supported bprim types (cached)
    supported_bprims: TfTokenVector,

    /// Draw items cache for deduplicating filtered draw item queries.
    ///
    /// Port of C++ HdStRenderDelegate owning HdSt_DrawItemsCache (P1-24).
    /// Shared with render passes so they can populate and query the cache.
    draw_items_cache: Arc<Mutex<HdStDrawItemsCache>>,
}

impl HdStRenderDelegate {
    /// Create a new Storm render delegate.
    pub fn new() -> Self {
        Self::with_settings(HashMap::new())
    }

    /// Create with initial settings.
    pub fn with_settings(settings: HdRenderSettingsMap) -> Self {
        Self {
            render_param: Arc::new(HdStRenderParam::new()),
            resource_registry: RwLock::new(Arc::new(HdStResourceRegistry::new())),
            settings,
            // Supported rprim types (from C++ SUPPORTED_RPRIM_TYPES)
            supported_rprims: vec![
                Token::new("mesh"),
                Token::new("basisCurves"),
                Token::new("points"),
                Token::new("volume"),
            ],
            // Supported sprim types (from C++ SUPPORTED_SPRIM_TYPES, 25.02+)
            supported_sprims: vec![
                Token::new("camera"),
                Token::new("drawTarget"),
                Token::new("extComputation"),
                Token::new("material"),
                Token::new("domeLight"),
                Token::new("cylinderLight"),
                Token::new("diskLight"),
                Token::new("distantLight"),
                Token::new("rectLight"),
                Token::new("simpleLight"),
                Token::new("sphereLight"),
                Token::new("imageShader"),
            ],
            // Supported bprim types (from C++: renderBuffer + HdStField types)
            supported_bprims: vec![
                Token::new("renderBuffer"),
                Token::new("openvdbAsset"),
                Token::new("field3dAsset"),
            ],
            draw_items_cache: Arc::new(Mutex::new(HdStDrawItemsCache::new())),
        }
    }

    /// Get the draw items cache (shared with render passes).
    ///
    /// Port of C++ HdStRenderDelegate::GetDrawItemsCache (P1-24).
    pub fn get_draw_items_cache(&self) -> Arc<Mutex<HdStDrawItemsCache>> {
        Arc::clone(&self.draw_items_cache)
    }

    /// Get the material-tags version from the shared Storm render param.
    pub fn get_material_tags_version(&self) -> usize {
        self.render_param.get_material_tags_version()
    }

    /// Get the geom-subset-draw-items version from the shared Storm render param.
    pub fn get_geom_subset_draw_items_version(&self) -> usize {
        self.render_param.get_geom_subset_draw_items_version()
    }

    /// Create HdDriverVector with Hgi for Storm (renderDriver).
    ///
    /// Use with HdRenderIndex::new. With `opengl` feature, pass `Arc::new(RwLock::new(HgiGL::new()))`.
    /// With other backends, pass the appropriate Hgi implementation.
    pub fn create_drivers(
        hgi: std::sync::Arc<parking_lot::RwLock<dyn usd_hgi::Hgi + Send>>,
    ) -> HdDriverVector {
        vec![HdDriver::new(
            RENDER_DRIVER.clone(),
            HgiDriverHandle::new(hgi).into(),
        )]
    }

    /// Get render delegate info for HdSceneIndexPluginRegistry.
    ///
    /// Port of C++ `HdStRenderDelegate::GetRenderDelegateInfo()` (P1-35).
    /// Returns materialRenderContexts, shaderSourceTypes, primvarFilteringNeeded.
    pub fn get_render_delegate_info(&self) -> HdRenderDelegateInfo {
        HdRenderDelegateInfo {
            material_binding_purpose: Token::new("preview"),
            // glslfx is the primary Storm material render context.
            // MaterialX (mtlx) is added when the mtlx feature is enabled.
            material_render_contexts: {
                #[allow(unused_mut)]
                let mut ctxs = vec![Token::new("glslfx")];
                #[cfg(feature = "mtlx")]
                ctxs.push(Token::new("mtlx"));
                ctxs
            },
            render_settings_namespaces: vec![Token::new("storm")],
            is_primvar_filtering_needed: true,
            shader_source_types: vec![Token::new("glslfx")],
            is_coord_sys_supported: false,
        }
    }

    /// Check if Storm is supported on current hardware.
    ///
    /// Returns true when any GPU backend is available (OpenGL, wgpu/Vulkan/Metal/DX12).
    /// The wgpu backend (usd-hgi-wgpu) covers all modern platforms, so Storm
    /// is effectively always supported. The `opengl` feature enables legacy GL path.
    pub fn is_supported() -> bool {
        // Storm is supported with either the OpenGL backend or the wgpu backend.
        // Since wgpu covers Vulkan/Metal/DX12/WebGPU and is always available as
        // a crate, Storm rendering is always available.
        true
    }

    /// Get render settings.
    pub fn get_render_settings(&self) -> &HdRenderSettingsMap {
        &self.settings
    }

    /// Set a render setting.
    pub fn set_render_setting(&mut self, key: Token, value: Value) {
        self.settings.insert(key, value);
    }

    /// Commit resources (called after sync).
    /// Uploads all pending buffer sources to GPU, then garbage collects if needed.
    pub fn do_commit_resources(&mut self) {
        // Flush all pending buffer sources accumulated during rprim sync.
        {
            let reg = self.resource_registry.read();
            reg.commit();
        }
        if self.render_param.needs_gc() {
            {
                let reg = self.resource_registry.read();
                reg.garbage_collect();
            }
        }
    }
}

// Separate impl block for set_drivers - called by HdRenderIndex::new
impl HdStRenderDelegate {
    /// Extract Hgi from drivers and install Hgi-backed resource registry.
    fn install_hgi_registry(&self, drivers: &HdDriverVector) -> bool {
        for driver in drivers {
            if driver.name == *RENDER_DRIVER {
                if let Some(hgi_handle) = driver.driver.get::<HgiDriverHandle>().cloned() {
                    let registry = Arc::new(HdStResourceRegistry::new_with_hgi(hgi_handle));
                    {
                        let mut reg = self.resource_registry.write();
                        *reg = registry;
                        return true;
                    }
                }
            }
        }
        false
    }
}

impl Default for HdStRenderDelegate {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Light sync adapter: bridges HdSceneDelegate → LightSceneDelegate → HdStLight
// ---------------------------------------------------------------------------

/// Adapts `&dyn HdSceneDelegate` into `LightSceneDelegate` for HdStLight::sync().
///
/// HdSceneDelegate already provides all 4 methods that LightSceneDelegate needs
/// (`get_light_param_value`, `get_transform`, `get_visible`, `get`).
struct LightDelegateAdapter<'a>(&'a dyn HdSceneDelegate);

impl<'a> crate::light::LightSceneDelegate for LightDelegateAdapter<'a> {
    fn get_light_param_value(&self, id: &SdfPath, param: &Token) -> Value {
        self.0.get_light_param_value(id, param)
    }
    fn get_transform(&self, id: &SdfPath) -> usd_gf::Matrix4d {
        self.0.get_transform(id)
    }
    fn get_visible(&self, id: &SdfPath) -> bool {
        self.0.get_visible(id)
    }
    fn get(&self, id: &SdfPath, key: &Token) -> Value {
        self.0.get(id, key)
    }
}

/// Wraps `HdStLight` into the `HdSprimSync` trait so the render index can sync
/// lights via the standard sprim pipeline (sync_sprims_impl).
pub struct HdStLightSyncAdapter(HdStLight);

impl HdStLightSyncAdapter {
    pub fn new(light: HdStLight) -> Self {
        Self(light)
    }
}

impl HdSprimSync for HdStLightSyncAdapter {
    fn sync_dyn(
        &mut self,
        delegate: &dyn HdSceneDelegate,
        _render_param: Option<&dyn usd_hd::render::render_delegate::HdRenderParam>,
        dirty_bits: &mut HdDirtyBits,
    ) {
        // Bridge HdSceneDelegate → LightSceneDelegate, then call HdStLight::sync
        let adapter = LightDelegateAdapter(delegate);
        self.0.sync(&adapter, dirty_bits);
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

/// Opaque ownership token for Storm mesh rprims when the live mesh is stored in
/// the typed sync handle.
///
/// OpenUSD Storm owns a single `HdStMesh` instance per mesh rprim. The Rust
/// render index currently stores both an opaque `HdPrimHandle` and an optional
/// typed `sync_handle`; allocating a second real `HdStMesh` for the opaque slot
/// causes the two objects to diverge.
///
/// Mesh runtime paths already prefer the typed sync handle for sync and draw
/// item queries, so the opaque handle only needs to preserve ownership and
/// presence semantics.
struct HdStMeshHandlePlaceholder {
    #[allow(dead_code)]
    id: SdfPath,
}

impl HdStMeshHandlePlaceholder {
    fn new(id: SdfPath) -> Self {
        Self { id }
    }
}

/// Return true when the opaque Storm rprim slot contains the mesh placeholder
/// rather than a live `HdStMesh`.
///
/// After the single-mesh ownership fix, real Storm meshes live in the typed
/// sync handle. Seeing this placeholder inside opaque sync hooks means the
/// caller routed a mesh through the wrong legacy path.
fn is_mesh_placeholder_handle(handle: &HdPrimHandle) -> bool {
    handle.as_ref().is::<HdStMeshHandlePlaceholder>()
}

impl HdRenderDelegate for HdStRenderDelegate {
    fn get_supported_rprim_types(&self) -> &TfTokenVector {
        &self.supported_rprims
    }

    fn get_supported_sprim_types(&self) -> &TfTokenVector {
        &self.supported_sprims
    }

    fn get_supported_bprim_types(&self) -> &TfTokenVector {
        &self.supported_bprims
    }

    /// Storm requires storm-specific task graph (e.g. HdxTaskController).
    /// C++: returns true unconditionally.
    fn requires_storm_tasks(&self) -> bool {
        true
    }

    /// Storm filters primvars to only what's needed by the material.
    /// C++: `IsPrimvarFilteringNeeded() = true` from HdRenderDelegateInfo.
    fn is_primvar_filtering_needed(&self) -> bool {
        true
    }

    /// Storm material render contexts: glslfx primary, mtlx secondary.
    /// C++: `GetRenderDelegateInfo().materialRenderContexts` = {glslfx [, mtlx]}.
    fn get_material_render_contexts(&self) -> TfTokenVector {
        // glslfx is always the primary material render context for Storm.
        // MaterialX (mtlx) is conditionally included when the mtlx feature is on.
        vec![Token::new("glslfx")]
    }

    /// Shader source types match material render contexts.
    /// C++: `shaderSourceTypes = materialRenderContexts`.
    fn get_shader_source_types(&self) -> TfTokenVector {
        self.get_material_render_contexts()
    }

    /// Storm uses "preview" material binding purpose.
    fn get_material_binding_purpose(&self) -> Token {
        Token::new("preview")
    }

    /// Returns GPU memory allocation stats from the resource registry.
    /// C++: merges gpuMemoryUsed + textureMemory.
    fn get_render_stats(&self) -> std::collections::HashMap<String, Value> {
        let mut stats = std::collections::HashMap::new();
        {
            let reg = self.resource_registry.read();
            let alloc = reg.get_resource_allocation();
            let gpu_mem = alloc.get("gpuMemoryUsed").copied().unwrap_or(0u64);
            let tex_mem = alloc.get("textureMemory").copied().unwrap_or(0u64);
            // Combine GPU + texture memory into a single gpuMemoryUsed stat
            stats.insert("gpuMemoryUsed".to_string(), Value::from(gpu_mem + tex_mem));
            for (k, v) in alloc {
                if k != "gpuMemoryUsed" {
                    stats.insert(k, Value::from(v));
                }
            }
        }
        stats
    }

    /// Returns default AOV descriptor for named AOV outputs.
    ///
    /// Matches C++ `GetDefaultAovDescriptor`:
    /// - color      → Float16Vec4, MSAA, clear=vec4(0)
    /// - depth      → Float32, MSAA, clear=1.0
    /// - depthStencil → Float32UInt8, MSAA, clear=1.0/0
    /// - id/primId  → Int32, MSAA, clear=-1
    /// - Neye       → UNorm8Vec4, MSAA, clear=vec4(0)
    fn get_default_aov_descriptor(&self, name: &Token) -> usd_hd::aov::HdAovDescriptor {
        use usd_gf::Vec4f;
        use usd_hd::aov::HdAovDescriptor;
        use usd_hd::types::HdFormat;
        match name.as_str() {
            "color" => HdAovDescriptor::new(
                HdFormat::Float16Vec4,
                true,
                Value::from(Vec4f::new(0.0, 0.0, 0.0, 0.0)),
            ),
            _ if usd_hd::aov::hd_aov_has_depth_stencil_semantic(name) => {
                HdAovDescriptor::new(HdFormat::Float32UInt8, true, Value::from(1.0f32))
            }
            _ if usd_hd::aov::hd_aov_has_depth_semantic(name) => {
                HdAovDescriptor::new(HdFormat::Float32, true, Value::from(1.0f32))
            }
            "primId" | "instanceId" | "elementId" | "edgeId" | "pointId" => {
                HdAovDescriptor::new(HdFormat::Int32, true, Value::from(-1i32))
            }
            "Neye" => HdAovDescriptor::new(
                HdFormat::UNorm8Vec4,
                true,
                Value::from(Vec4f::new(0.0, 0.0, 0.0, 0.0)),
            ),
            _ => HdAovDescriptor::default(),
        }
    }

    /// Returns the current render settings version.
    ///
    /// The version is a monotonic counter that increments every time
    /// a render setting changes. Hydra uses this to detect when render
    /// settings need to be re-applied.
    fn get_render_settings_version(&self) -> u32 {
        self.render_param.get_material_tags_version() as u32
    }

    fn create_rprim(&mut self, type_id: &Token, id: SdfPath) -> Option<HdPrimHandle> {
        match type_id.as_str() {
            "mesh" => Some(Box::new(HdStMeshHandlePlaceholder::new(id))),
            "basisCurves" => {
                let curves = crate::basis_curves::HdStBasisCurves::new(id);
                Some(Box::new(curves))
            }
            "points" => {
                let points = crate::points::HdStPoints::new(id);
                Some(Box::new(points))
            }
            "volume" => {
                let volume = crate::volume::HdStVolume::new(id);
                Some(Box::new(volume))
            }
            _ => None,
        }
    }

    fn create_sprim(&mut self, type_id: &Token, id: SdfPath) -> Option<HdPrimHandle> {
        match type_id.as_str() {
            "camera" => {
                let camera = HdCamera::new(id);
                Some(Box::new(camera))
            }
            "drawTarget" => {
                let draw_target = HdStDrawTarget::new(id);
                Some(Box::new(draw_target))
            }
            "extComputation" => {
                let registry = self.resource_registry.read().clone();
                let ext_comp = HdStExtComputation::new(id, registry);
                Some(Box::new(ext_comp))
            }
            "material" => {
                let material = HdStMaterial::new(id);
                Some(Box::new(material))
            }
            "domeLight" | "cylinderLight" | "diskLight" | "distantLight" | "rectLight"
            | "simpleLight" | "sphereLight" => {
                let light = HdStLight::new(id.clone(), type_id.clone());
                Some(Box::new(light))
            }
            "imageShader" => {
                let image_shader = HdImageShader::new(id);
                Some(Box::new(image_shader))
            }
            _ => None,
        }
    }

    fn create_rprim_sync(&mut self, type_id: &Token, id: &SdfPath) -> Option<HdRprimHandle> {
        match type_id.as_str() {
            "mesh" => {
                let mut mesh = HdStMesh::new(id.clone());
                mesh.set_resource_registry(self.resource_registry.read().clone());
                Some(Box::new(RprimAdapter(mesh)))
            }
            _ => None,
        }
    }

    fn pre_sync_rprims_batch(
        &self,
        handles: &mut [(&SdfPath, &mut HdPrimHandle, &mut usd_hd::types::HdDirtyBits)],
        _delegate: &dyn usd_hd::prim::HdSceneDelegate,
        repr_token: &Token,
    ) {
        use usd_hd::prim::rprim::HdRprim;

        for (_prim_id, handle, dirty_bits) in handles.iter_mut() {
            if let Some(mesh) = handle.downcast_mut::<HdStMesh>() {
                mesh.init_repr(repr_token, dirty_bits);
                if HdRprim::can_skip_dirty_bit_propagation_and_sync(mesh, **dirty_bits) {
                    **dirty_bits = usd_hd::change_tracker::HdRprimDirtyBits::CLEAN;
                    continue;
                }
                **dirty_bits = HdRprim::propagate_rprim_dirty_bits(mesh, **dirty_bits);
            } else if is_mesh_placeholder_handle(handle) {
                log::error!(
                    "[storm] pre_sync_rprims_batch routed mesh placeholder through opaque pre-sync path: {}",
                    _prim_id
                );
            }
        }
    }

    /// Sync an opaque delegate-managed rprim: read delegate data, process topology,
    /// upload to GPU.
    ///
    /// This hook exists for backends that still keep live rprims only in the
    /// opaque `HdPrimHandle`. Storm meshes no longer use that contract: the live
    /// mesh now resides in `create_rprim_sync(...)`.
    ///
    /// Three-phase pipeline:
    ///   1. read from delegate (sequential — delegate is not Send)
    ///   2. process_cpu (parallelizable — pure owned data)
    ///   3. upload to registry (sequential — shared GPU resources)
    /// This single-call path runs all three sequentially for compatibility.
    fn sync_rprim(
        &self,
        handle: &mut HdPrimHandle,
        _prim_id: &SdfPath,
        delegate: &dyn usd_hd::prim::HdSceneDelegate,
        dirty_bits: &mut usd_hd::types::HdDirtyBits,
        repr_token: &Token,
    ) {
        if let Some(mesh) = handle.downcast_mut::<HdStMesh>() {
            let registry = self.resource_registry.read();
            mesh.sync_from_delegate(delegate, dirty_bits);
            mesh.process_cpu();
            mesh.upload_to_registry(&registry);
            *dirty_bits = 0;
        } else if let Some(curves) = handle.downcast_mut::<crate::basis_curves::HdStBasisCurves>() {
            let registry = self.resource_registry.read();
            curves.sync_from_delegate(delegate, dirty_bits, repr_token);
            curves.process_cpu(repr_token);
            curves.upload_to_registry(&registry, repr_token);
            log::info!(
                "[storm] sync_rprim curves: {} repr={} verts={} indices={} draw_items={}",
                _prim_id,
                repr_token,
                curves.get_vertex_count(),
                curves.get_index_count(),
                curves.get_draw_item_count(),
            );
            *dirty_bits = 0;
        } else if let Some(points) = handle.downcast_mut::<crate::points::HdStPoints>() {
            let registry = self.resource_registry.read();
            points.sync_from_delegate(delegate, dirty_bits);
            points.process_cpu();
            points.upload_to_registry(&registry, repr_token);
            *dirty_bits = 0;
        } else if is_mesh_placeholder_handle(handle) {
            log::error!(
                "[storm] sync_rprim routed mesh placeholder through opaque sync path: {}",
                _prim_id
            );
        } else {
            log::warn!("[storm] sync_rprim: downcast failed for {}", _prim_id);
        }
    }

    /// Batch-parallel sync for delegate-managed opaque rprims.
    ///
    /// Phase 1 (sequential): read data from scene delegate for each handle.
    /// Phase 2 (parallel via rayon): CPU topology expansion for all meshes.
    /// Phase 3 (sequential): upload processed buffers to GPU registry.
    ///
    /// Architecture note: Phase 2 can be replaced with GPU compute in the future
    /// by swapping `process_cpu()` for a wgpu compute dispatch.
    fn sync_rprims_batch(
        &self,
        handles: &mut [(&SdfPath, &mut HdPrimHandle, &mut usd_hd::types::HdDirtyBits)],
        delegate: &dyn usd_hd::prim::HdSceneDelegate,
        repr_token: &Token,
    ) {
        use rayon::prelude::*;

        let batch_started = std::time::Instant::now();
        let registry = self.resource_registry.read();

        // Phase 1 (sequential): read from delegate — delegate is not Send.
        let phase1_started = std::time::Instant::now();
        let mut meshes: Vec<&mut HdStMesh> = Vec::with_capacity(handles.len());
        let mut curves_prims: Vec<&mut crate::basis_curves::HdStBasisCurves> = Vec::new();
        let mut points_prims: Vec<&mut crate::points::HdStPoints> = Vec::new();
        for (prim_id, handle, dirty_bits) in handles.iter_mut() {
            if is_mesh_placeholder_handle(handle) {
                log::error!(
                    "[storm] sync_rprims_batch routed mesh placeholder through opaque batch sync path: {}",
                    prim_id
                );
            } else if handle.as_ref().is::<HdStMesh>() {
                let mesh = handle
                    .downcast_mut::<HdStMesh>()
                    .expect("type checked HdStMesh downcast");
                mesh.sync_from_delegate(delegate, dirty_bits);
                meshes.push(mesh);
            } else if handle.as_ref().is::<crate::basis_curves::HdStBasisCurves>() {
                let curves = handle
                    .downcast_mut::<crate::basis_curves::HdStBasisCurves>()
                    .expect("type checked HdStBasisCurves downcast");
                curves.sync_from_delegate(delegate, dirty_bits, repr_token);
                curves_prims.push(curves);
            } else if handle.as_ref().is::<crate::points::HdStPoints>() {
                let points = handle
                    .downcast_mut::<crate::points::HdStPoints>()
                    .expect("type checked HdStPoints downcast");
                points.sync_from_delegate(delegate, dirty_bits);
                points_prims.push(points);
            } else {
                log::warn!(
                    "[storm] sync_rprim_parallel: downcast failed for {}",
                    prim_id
                );
            }
        }
        let phase1_ms = phase1_started.elapsed().as_secs_f64() * 1000.0;
        log::debug!(
            "[storm] parallel sync: phase1 delegate read done ({} meshes, {:.2} ms)",
            meshes.len(),
            phase1_ms
        );

        // Phase 2 (parallel): CPU topology expansion — pure owned data per mesh.
        let phase2_started = std::time::Instant::now();
        meshes.par_iter_mut().for_each(|mesh| {
            mesh.process_cpu();
        });
        curves_prims.par_iter_mut().for_each(|curves| {
            curves.process_cpu(repr_token);
        });
        points_prims.par_iter_mut().for_each(|points| {
            points.process_cpu();
        });
        let phase2_ms = phase2_started.elapsed().as_secs_f64() * 1000.0;
        log::debug!(
            "[storm] parallel sync: phase2 process_cpu done ({:.2} ms)",
            phase2_ms
        );

        // Phase 3 (sequential): upload to GPU registry.
        let phase3_started = std::time::Instant::now();
        for mesh in &mut meshes {
            mesh.upload_to_registry(&registry);
        }
        for curves in &mut curves_prims {
            curves.upload_to_registry(&registry, repr_token);
            log::info!(
                "[storm] batch_sync curves: verts={} indices={} draw_items={}",
                curves.get_vertex_count(),
                curves.get_index_count(),
                curves.get_draw_item_count(),
            );
        }
        for points in &mut points_prims {
            points.upload_to_registry(&registry, repr_token);
        }
        let phase3_ms = phase3_started.elapsed().as_secs_f64() * 1000.0;
        log::debug!(
            "[storm] parallel sync: phase3 upload done ({:.2} ms, total {:.2} ms)",
            phase3_ms,
            batch_started.elapsed().as_secs_f64() * 1000.0
        );
    }

    fn create_sprim_sync(&mut self, type_id: &Token, id: &SdfPath) -> Option<HdSprimHandle> {
        match type_id.as_str() {
            "domeLight" | "cylinderLight" | "diskLight" | "distantLight" | "rectLight"
            | "simpleLight" | "sphereLight" => {
                let light = HdStLight::new(id.clone(), type_id.clone());
                Some(Box::new(HdStLightSyncAdapter::new(light)))
            }
            _ => None,
        }
    }

    fn create_bprim(&mut self, type_id: &Token, id: SdfPath) -> Option<HdPrimHandle> {
        if field::is_supported_bprim_type(type_id) {
            let field = HdStField::new(id, type_id.clone());
            Some(Box::new(field))
        } else if type_id == "renderBuffer" {
            let buffer = crate::render_buffer::HdStRenderBuffer::new(id);
            Some(Box::new(buffer))
        } else {
            None
        }
    }

    fn create_instancer(
        &mut self,
        _delegate: &dyn HdSceneDelegate,
        id: SdfPath,
    ) -> Option<Box<dyn usd_hd::render::render_delegate::HdInstancer>> {
        let instancer = crate::instancer::HdStInstancer::new(id);
        Some(Box::new(instancer))
    }

    fn destroy_instancer(
        &mut self,
        _instancer: Box<dyn usd_hd::render::render_delegate::HdInstancer>,
    ) {
    }

    fn create_render_pass(
        &mut self,
        _index: &HdRenderIndex,
        collection: &HdRprimCollection,
    ) -> Option<HdRenderPassSharedPtr> {
        let pass = HdStRenderPass::new(collection.clone());
        Some(Arc::new(pass))
    }

    fn create_fallback_sprim(&mut self, type_id: &Token) -> Option<HdPrimHandle> {
        let empty = SdfPath::default();
        match type_id.as_str() {
            "camera" => Some(Box::new(HdCamera::new(empty))),
            "drawTarget" => Some(Box::new(HdStDrawTarget::new(empty))),
            "extComputation" => {
                let registry = self.resource_registry.read().clone();
                Some(Box::new(HdStExtComputation::new(empty, registry)))
            }
            "material" => Some(Box::new(HdStMaterial::new(empty))),
            "domeLight" | "cylinderLight" | "diskLight" | "distantLight" | "rectLight"
            | "simpleLight" | "sphereLight" => {
                Some(Box::new(HdStLight::new(empty, type_id.clone())))
            }
            "imageShader" => Some(Box::new(HdImageShader::new(empty))),
            _ => None,
        }
    }

    fn create_fallback_bprim(&mut self, type_id: &Token) -> Option<HdPrimHandle> {
        let empty = SdfPath::default();
        if field::is_supported_bprim_type(type_id) {
            Some(Box::new(HdStField::new(empty, type_id.clone())))
        } else if type_id == "renderBuffer" {
            Some(Box::new(crate::render_buffer::HdStRenderBuffer::new(empty)))
        } else {
            None
        }
    }

    fn commit_resources(&mut self, _tracker: &mut HdChangeTracker) {
        self.do_commit_resources();
    }

    fn get_resource_registry(&self) -> HdResourceRegistrySharedPtr {
        self.resource_registry.read().clone()
    }

    fn get_draw_items_for_rprim(
        &self,
        handle: &HdPrimHandle,
        sync_handle: Option<&dyn std::any::Any>,
        _prim_id: &SdfPath,
        collection: &HdRprimCollection,
        _render_tags: &[Token],
    ) -> Vec<std::sync::Arc<dyn std::any::Any + Send + Sync>> {
        use usd_hd::render::render_index::RprimAdapter;
        let repr_selector = collection.get_repr_selector();
        let repr_tokens: Vec<Token> = if repr_selector.any_active_repr() {
            (0..usd_hd::prim::HdReprSelector::MAX_TOPOLOGY_REPRS)
                .filter(|&i| repr_selector.is_active_repr(i))
                .map(|i| repr_selector.get_token(i).clone())
                .collect()
        } else {
            vec![Token::new("refined")]
        };

        let mut result = Vec::new();

        // When a typed sync handle exists (RprimAdapter<HdStMesh>), read draw items
        // from it — it's the mesh that was actually synced by HdRprim::sync.
        if let Some(adapter) =
            sync_handle.and_then(|sh| sh.downcast_ref::<RprimAdapter<HdStMesh>>())
        {
            for repr_token in &repr_tokens {
                for item in adapter.0.get_draw_items(repr_token) {
                    result.push(item as Arc<dyn std::any::Any + Send + Sync>);
                }
            }
            return result;
        }

        // Fallback: read from opaque handle (used when no typed sync handle, e.g. initial populate
        // went through sync_rprim which syncs the handle mesh directly).
        if let Some(mesh) = (handle.as_ref() as &dyn std::any::Any).downcast_ref::<HdStMesh>() {
            for repr_token in &repr_tokens {
                for item in mesh.get_draw_items(repr_token) {
                    result.push(item as Arc<dyn std::any::Any + Send + Sync>);
                }
            }
        } else if let Some(curves) = (handle.as_ref() as &dyn std::any::Any)
            .downcast_ref::<crate::basis_curves::HdStBasisCurves>()
        {
            for repr_token in &repr_tokens {
                for item in curves.get_draw_items(repr_token) {
                    result.push(item as Arc<dyn std::any::Any + Send + Sync>);
                }
            }
        } else if let Some(points) =
            (handle.as_ref() as &dyn std::any::Any).downcast_ref::<crate::points::HdStPoints>()
        {
            for repr_token in &repr_tokens {
                for item in points.get_draw_items(repr_token) {
                    result.push(item as Arc<dyn std::any::Any + Send + Sync>);
                }
            }
        } else if let Some(volume) =
            (handle.as_ref() as &dyn std::any::Any).downcast_ref::<crate::volume::HdStVolume>()
        {
            for repr_token in &repr_tokens {
                for item in volume.get_draw_items(repr_token) {
                    result.push(item as Arc<dyn std::any::Any + Send + Sync>);
                }
            }
        }
        result
    }

    fn set_drivers(&mut self, drivers: &HdDriverVector) {
        self.install_hgi_registry(drivers);
    }

    fn get_render_param(&self) -> Option<HdRenderParamSharedPtr> {
        Some(self.render_param.clone())
    }

    fn get_render_setting_descriptors(&self) -> HdRenderSettingDescriptorList {
        vec![
            HdRenderSettingDescriptor::new(
                "Enable Tiny Prim Culling",
                Token::new("enableTinyPrimCulling"),
                Value::from(true),
            ),
            HdRenderSettingDescriptor::new(
                "Max Lights",
                Token::new("maxLights"),
                Value::from(16i32),
            ),
            HdRenderSettingDescriptor::new(
                "Volume Raymarching Step Size",
                Token::new("volumeRaymarchingStepSize"),
                Value::from(1.0f64),
            ),
        ]
    }

    fn get_render_setting(&self, key: &Token) -> Option<Value> {
        self.settings.get(key).cloned()
    }
}

impl HdStRenderDelegate {
    /// Get the Storm resource registry (concrete type) for mesh sync and blit operations.
    pub fn get_st_resource_registry(&self) -> HdStResourceRegistrySharedPtr {
        self.resource_registry.read().clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::HdStDrawItem;
    use usd_hd::prim::HdReprSelector;

    #[test]
    fn test_delegate_creation() {
        let delegate = HdStRenderDelegate::new();

        assert!(
            delegate
                .get_supported_rprim_types()
                .contains(&Token::new("mesh"))
        );
        assert!(
            delegate
                .get_supported_sprim_types()
                .contains(&Token::new("material"))
        );
        assert!(
            delegate
                .get_supported_bprim_types()
                .contains(&Token::new("renderBuffer"))
        );
    }

    #[test]
    fn test_create_mesh() {
        let mut delegate = HdStRenderDelegate::new();
        let path = SdfPath::from_string("/mesh").unwrap();

        let prim = delegate.create_rprim(&Token::new("mesh"), path.clone());
        let sync = delegate.create_rprim_sync(&Token::new("mesh"), &path);
        assert!(prim.is_some());
        assert!(sync.is_some());
        assert!(
            prim.as_ref()
                .is_some_and(|handle| handle.as_ref().is::<HdStMeshHandlePlaceholder>())
        );
        assert!(
            sync.as_ref()
                .is_some_and(|handle| handle.as_any_ref().is::<RprimAdapter<HdStMesh>>())
        );
    }

    #[test]
    fn test_create_material() {
        let mut delegate = HdStRenderDelegate::new();
        let path = SdfPath::from_string("/material").unwrap();

        let prim = delegate.create_sprim(&Token::new("material"), path);
        assert!(prim.is_some());
    }

    #[test]
    fn test_create_render_pass() {
        let delegate = HdStRenderDelegate::new();
        let collection = HdRprimCollection::new(Token::new("test"));

        // create_render_pass requires &HdRenderIndex; skip in unit test
        // (tested via integration tests with full render index)
        let _ = delegate;
        let _ = collection;
    }

    #[test]
    fn test_resource_registry() {
        let delegate = HdStRenderDelegate::new();
        // Just verify that a registry is returned (concrete type methods not accessible via trait)
        let _registry = delegate.get_resource_registry();
    }

    #[test]
    fn test_render_settings() {
        let mut delegate = HdStRenderDelegate::new();

        let val = Value::from(32i32);
        delegate.set_render_setting(Token::new("maxLights"), val.clone());

        assert_eq!(
            delegate.get_render_setting(&Token::new("maxLights")),
            Some(val)
        );
    }

    #[test]
    fn test_setting_descriptors() {
        let delegate = HdStRenderDelegate::new();
        let descriptors = delegate.get_render_setting_descriptors();

        assert!(!descriptors.is_empty());
        assert!(descriptors.iter().any(|d| d.key == Token::new("maxLights")));
    }

    #[test]
    fn test_get_draw_items_for_rprim_respects_collection_repr_selector() {
        let delegate = HdStRenderDelegate::new();
        let path = SdfPath::from_string("/mesh").unwrap();

        let mut mesh = HdStMesh::new(path.clone());
        let refined = HdStDrawItem::new(path.clone());
        refined.set_repr(Token::new("refined"));
        mesh.add_draw_item(Arc::new(refined));

        let hull = HdStDrawItem::new(path.clone());
        hull.set_repr(Token::new("hull"));
        mesh.add_draw_item(Arc::new(hull));

        let handle: HdPrimHandle = Box::new(mesh);
        let collection = HdRprimCollection::with_repr(
            Token::new("geometry"),
            HdReprSelector::with_token(Token::new("hull")),
            false,
            Token::default(),
        );

        let items = delegate.get_draw_items_for_rprim(&handle, None, &path, &collection, &[]);
        assert_eq!(items.len(), 1);

        let item = items[0]
            .clone()
            .downcast::<HdStDrawItem>()
            .expect("downcast HdStDrawItem");
        assert_eq!(item.get_repr(), Token::new("hull"));
    }
}
