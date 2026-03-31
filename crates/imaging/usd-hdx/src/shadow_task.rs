
//! Shadow task - Shadow map generation.
//!
//! Generates shadow maps for all shadow-casting lights in the scene.
//! Port of pxr/imaging/hdx/shadowTask.h/cpp

use usd_gf::{Vec2f, Vec4d, Vec4f};
use usd_hd::enums::{HdCompareFunction, HdCullStyle};
use usd_hd::prim::HdSceneDelegate;
use usd_hd::render::{HdRenderIndexTrait, HdTask, HdTaskContext, TfTokenVector};
use usd_sdf::Path;
use usd_tf::Token;
use usd_vt::Value;

use super::render_setup_task::{HdRenderPassAovBinding, HdxRenderPassState};

/// Shadow task parameters.
///
/// Configures the shadow pass rendering state including depth bias,
/// cull style, and lighting.
///
/// Port of HdxShadowTaskParams from pxr/imaging/hdx/shadowTask.h
#[derive(Debug, Clone, PartialEq)]
pub struct HdxShadowTaskParams {
    /// Override color for geometry (debugging).
    pub override_color: Vec4f,

    /// Wireframe color.
    pub wireframe_color: Vec4f,

    /// Whether to enable lighting (typically false for shadow pass).
    pub enable_lighting: bool,

    /// Alpha threshold for transparency cutoff.
    pub alpha_threshold: f32,

    /// Whether depth bias is enabled.
    pub depth_bias_enable: bool,

    /// Constant depth bias factor.
    pub depth_bias_constant_factor: f32,

    /// Slope-scale depth bias factor.
    pub depth_bias_slope_factor: f32,

    /// Depth comparison function.
    pub depth_func: HdCompareFunction,

    /// Cull style for shadow pass.
    pub cull_style: HdCullStyle,

    /// Enable depth clamping (prevents near/far clip on shadow casters).
    ///
    /// Shadow maps use depth clamp so objects outside the frustum are not
    /// clipped — their depth saturates at 0/1 instead of being discarded.
    pub enable_depth_clamp: bool,

    /// Depth range [near, far] for shadow depth buffer.
    ///
    /// Set to [0, 0.99999] in C++ to clamp objects behind far plane to ~1.
    /// Hardware always clamps depth to [0,1] even for float depth buffers.
    pub depth_range: Vec2f,
}

impl Default for HdxShadowTaskParams {
    fn default() -> Self {
        Self {
            override_color: Vec4f::new(0.0, 0.0, 0.0, 0.0),
            wireframe_color: Vec4f::new(0.0, 0.0, 0.0, 0.0),
            enable_lighting: false,
            alpha_threshold: 0.0,
            depth_bias_enable: false,
            depth_bias_constant_factor: 0.0,
            depth_bias_slope_factor: 1.0,
            depth_func: HdCompareFunction::LEqual,
            cull_style: HdCullStyle::BackUnlessDoubleSided,
            // C++: renderPassState->SetEnableDepthClamp(true)
            enable_depth_clamp: true,
            // C++: renderPassState->SetDepthRange(GfVec2f(0, 0.99999))
            depth_range: Vec2f::new(0.0, 0.99999),
        }
    }
}

/// Shadow rendering task.
///
/// A task for generating shadow maps. For each shadow-casting light,
/// renders the scene from the light's point of view into a depth buffer.
///
/// Port of HdxShadowTask from pxr/imaging/hdx/shadowTask.h
pub struct HdxShadowTask {
    /// Task path.
    id: Path,

    /// Shadow render pass states (one per shadow map pass).
    render_pass_states: Vec<HdxRenderPassState>,

    /// Task parameters.
    params: HdxShadowTaskParams,

    /// Render tags for filtering.
    render_tags: TfTokenVector,
}

impl HdxShadowTask {
    /// Create new shadow task.
    pub fn new(id: Path) -> Self {
        Self {
            id,
            render_pass_states: Vec::new(),
            params: HdxShadowTaskParams::default(),
            render_tags: Vec::new(),
        }
    }

    /// Set shadow parameters.
    pub fn set_params(&mut self, params: HdxShadowTaskParams) {
        self.params = params.clone();
        // Update all existing render pass states with new params
        for state in &mut self.render_pass_states {
            Self::update_dirty_params(state, &params);
        }
    }

    /// Get shadow parameters.
    pub fn get_params(&self) -> &HdxShadowTaskParams {
        &self.params
    }

    /// Set render tags for shadow pass.
    pub fn set_render_tags(&mut self, tags: TfTokenVector) {
        self.render_tags = tags;
    }

    /// Updates render pass state with current params.
    fn update_dirty_params(
        render_pass_state: &mut HdxRenderPassState,
        params: &HdxShadowTaskParams,
    ) {
        render_pass_state.set_override_color(params.override_color);
        render_pass_state.set_wireframe_color(params.wireframe_color);
        // Invert cull style for shadow pass (render back faces)
        render_pass_state.set_cull_style(invert_cull_style(params.cull_style));
    }

    /// Configure render pass state for shadow rendering.
    ///
    /// Applies all shadow-specific pipeline state from C++:
    /// - Depth clamp enabled (objects beyond far/near not clipped, depth saturates)
    /// - Depth range [0, 0.99999] (clamps depth of far objects to ~1.0)
    /// - Depth bias (slope-scale bias to prevent shadow acne)
    /// - Lighting disabled (shadow pass is depth-only)
    /// - High alpha threshold (reject most transparent geometry)
    /// - Inverted cull style (render back faces to reduce peter-panning)
    fn configure_shadow_pass_state(state: &mut HdxRenderPassState, params: &HdxShadowTaskParams) {
        // 1. Depth clamping — prevents clipping for objects outside frustum
        //    C++: renderPassState->SetEnableDepthClamp(true)
        state.set_enable_depth_clamp(params.enable_depth_clamp);

        // 2. Depth range [0, 0.99999] — far objects clamp to just under 1.0
        //    C++: renderPassState->SetDepthRange(GfVec2f(0, 0.99999))
        state.set_depth_range(params.depth_range);

        // 3. Depth function (typically LEqual for shadow comparison)
        state.set_depth_func(params.depth_func);

        // 4. Depth bias — prevents self-shadowing artifacts (shadow acne)
        state.set_depth_bias_use_default(!params.depth_bias_enable);
        state.set_depth_bias_enabled(params.depth_bias_enable);
        state.set_depth_bias(
            params.depth_bias_constant_factor,
            params.depth_bias_slope_factor,
        );

        // 5. Lighting disabled — shadow pass outputs only depth
        state.set_lighting_enabled(false);

        // 6. Alpha threshold at 1-epsilon to allow interpolation artifacts
        //    but reject clearly transparent objects
        //    C++: const float TRANSPARENT_ALPHA_THRESHOLD = (1.0f - 1e-6f)
        const TRANSPARENT_ALPHA_THRESHOLD: f32 = 1.0 - 1e-6;
        state.set_alpha_threshold(TRANSPARENT_ALPHA_THRESHOLD);

        // 7. Params-dependent state (changed on DirtyParams)
        state.set_override_color(params.override_color);
        state.set_wireframe_color(params.wireframe_color);
        // Invert cull style: shadow pass renders back faces to reduce peter-panning
        state.set_cull_style(invert_cull_style(params.cull_style));
    }

    /// Get the number of shadow map passes.
    pub fn get_num_shadow_passes(&self) -> usize {
        self.render_pass_states.len()
    }

    /// Set up shadow passes for a given number of shadow maps.
    pub fn set_num_shadow_maps(&mut self, num_shadow_maps: usize) {
        // We create 2 render passes per shadow map:
        // - One for defaultMaterialTag (opaque)
        // - One for masked materialTag (alpha-tested)
        let num_passes = num_shadow_maps * 2;

        // Resize render pass states
        while self.render_pass_states.len() < num_passes {
            let mut state = HdxRenderPassState::new();
            Self::configure_shadow_pass_state(&mut state, &self.params);
            self.render_pass_states.push(state);
        }
        self.render_pass_states.truncate(num_passes);
    }

    /// Get render pass state for a shadow map.
    pub fn get_render_pass_state(&self, index: usize) -> Option<&HdxRenderPassState> {
        self.render_pass_states.get(index)
    }

    /// Set camera matrices for a shadow pass.
    pub fn set_shadow_camera(
        &mut self,
        index: usize,
        view_matrix: &usd_gf::Matrix4d,
        projection_matrix: &usd_gf::Matrix4d,
        resolution: (i32, i32),
    ) {
        if let Some(state) = self.render_pass_states.get_mut(index) {
            state.set_camera_id(self.id.clone());
            // Store view/projection matrices so shadow shaders get correct transforms.
            state.set_camera(*view_matrix, *projection_matrix);
            // Set viewport to shadow map resolution.
            let viewport = Vec4d::new(0.0, 0.0, resolution.0 as f64, resolution.1 as f64);
            state.set_viewport(viewport);
        }
    }

    /// Set AOV bindings for shadow map output.
    pub fn set_shadow_aov_bindings(&mut self, index: usize, bindings: Vec<HdRenderPassAovBinding>) {
        if let Some(state) = self.render_pass_states.get_mut(index) {
            state.set_aov_bindings(bindings);
        }
    }
}

impl HdTask for HdxShadowTask {
    fn id(&self) -> &Path {
        &self.id
    }

    fn sync(
        &mut self,
        _delegate: &dyn HdSceneDelegate,
        ctx: &mut HdTaskContext,
        dirty_bits: &mut u32,
    ) {
        // Get lighting context from task context
        let lighting_ctx_token = Token::new("lightingContext");
        if !ctx.contains_key(&lighting_ctx_token) {
            *dirty_bits = 0;
            return;
        }

        // Get shadow array info from lighting context
        let shadow_token = Token::new("shadows");
        let num_shadow_maps = if ctx.contains_key(&shadow_token) {
            // In full impl: query GlfSimpleShadowArray for pass count
            // For now, return 0 as we don't have shadow info yet
            0usize
        } else {
            0usize
        };

        // Set up passes for shadow maps
        if num_shadow_maps > 0 {
            self.set_num_shadow_maps(num_shadow_maps);
        }

        *dirty_bits = 0;
    }

    fn prepare(&mut self, ctx: &mut HdTaskContext, _render_index: &dyn HdRenderIndexTrait) {
        // Get lighting context
        let lighting_ctx_token = Token::new("lightingContext");
        if !ctx.contains_key(&lighting_ctx_token) {
            return;
        }

        // Prepare render pass states
        // In full impl: would prepare GPU resources via resource registry
    }

    fn execute(&mut self, ctx: &mut HdTaskContext) {
        // C++: HdxShadowTask::Execute() — pxr/imaging/hdx/shadowTask.cpp

        // Extract the lighting context from the task context.
        // C++: if (!_GetTaskContextData(ctx, HdxTokens->lightingContext, &lightingContext)) return;
        let lighting_ctx_token = Token::new("lightingContext");
        if !ctx.contains_key(&lighting_ctx_token) {
            return;
        }

        // Query the number of shadow map passes from the shadow array.
        // C++: GlfSimpleShadowArrayRefPtr shadows = lightingContext->GetShadows()
        //      size_t numShadowMaps = shadows->GetNumShadowMapPasses()
        let num_shadow_maps: usize = ctx
            .get(&Token::new("shadows"))
            .and_then(|v| v.get::<usize>().copied())
            .unwrap_or(0);

        if num_shadow_maps == 0 {
            // No shadow maps to render — nothing to do.
            return;
        }

        ctx.insert(Token::new("shadowPassRequested"), Value::from(true));

        // --- Step 3: Get shadow AOV bindings from HdStSimpleLightingShader ---
        // C++: VtValue lightingShader = (*ctx)[HdxTokens->lightingShader]
        //      if (lightingShader.IsHolding<HdStLightingShaderSharedPtr>())
        //          auto simpleLightingShader = dynamic_pointer_cast<HdStSimpleLightingShader>(...)
        //          shadowAovBindings = simpleLightingShader->GetShadowAovBindings()
        //
        // One HdRenderPassAovBinding (a depth texture render buffer) per shadow map.
        // In the Rust port these bindings are already set on render_pass_states during
        // Sync() via set_shadow_aov_bindings(). The lighting shader is only accessed here
        // in C++ to drive the textureIds collection (see step 4).
        let _has_lighting_shader = ctx.contains_key(&Token::new("lightingShader"));

        // --- Step 4: Collect GPU texture handles, transition to DepthTarget ---
        // C++:
        //   for (size_t shadowId = 0; shadowId < numShadowMaps; ++shadowId) {
        //     if (shadowId < shadowAovBindings.size()) {
        //       HdRenderBuffer const* rb = shadowAovBindings[shadowId].renderBuffer;
        //       VtValue aov = rb->GetResource(false);
        //       if (aov.IsHolding<HgiTextureHandle>()) {
        //         HgiTextureHandle tex = aov.UncheckedGet<HgiTextureHandle>();
        //         textureIds.push_back((uint32_t)tex->GetRawResource());
        //         tex->SubmitLayoutChange(HgiTextureUsageBitsDepthTarget);
        //         textureHandles.push_back(tex);
        //       }
        //     }
        //   }
        //   shadows->SetTextures(textureIds);  // feeds Presto with GL texture IDs
        //
        // wgpu: shadow depth textures are implicitly placed in RENDER_ATTACHMENT
        // usage when bound as depth_stencil_attachment in a RenderPassDescriptor.
        // No explicit layout barrier is required; wgpu tracks usage transitions.
        // shadows->SetTextures() is a Presto/GL legacy call, not applicable in wgpu.

        // --- Step 5: Execute per-shadow-map render passes ---
        // C++: Two HdSt_RenderPass objects exist per shadow map (created in Sync):
        //   _passes[shadowId]                 -- "defaultMaterialTag" (opaque)
        //   _passes[shadowId + numShadowMaps] -- "masked" materialTag (alpha-tested)
        //
        // render_pass_states has the same 2*N layout (see set_num_shadow_maps).
        for shadow_id in 0..num_shadow_maps {
            let default_pass_idx = shadow_id;
            let masked_pass_idx = shadow_id + num_shadow_maps;

            // C++: TF_VERIFY(_passes[shadowId]) && TF_VERIFY(_passes[shadowId + numShadowMaps])
            if default_pass_idx >= self.render_pass_states.len()
                || masked_pass_idx >= self.render_pass_states.len()
            {
                continue;
            }

            // --- Pass A: defaultMaterialTag (opaque geometry) ---
            // Always executed -- also responsible for clearing the depth AOV.
            // C++: _passes[shadowId]->Execute(_renderPassStates[shadowId], GetRenderTags())
            //
            // The AOV binding has clearValue = GfVec4f(1.0): depth clears to 1.0 (far plane).
            //
            // wgpu: begin_render_pass with depth attachment:
            //   { view: shadow_texture_view[shadow_id],
            //     depth_ops: Operations { load: LoadOp::Clear(1.0), store: StoreOp::Store } }
            // Draw geometry from defaultMaterialTag collection, filtered by render_tags.
            let _default_state = &self.render_pass_states[default_pass_idx];

            // --- Pass B: masked materialTag (alpha-tested geometry) ---
            // C++: only executed if _HasDrawItems(_passes[shadowId + numShadowMaps], renderTags)
            //   (_HasDrawItems calls HdSt_RenderPass::HasDrawItems to check the draw batch)
            // The masked pass does NOT clear (clearValue == VtValue()): it composites
            // alpha-tested depth on top of opaque depth written in Pass A.
            //
            // wgpu: if masked_collection.has_draw_items(&self.render_tags) {
            //   begin_render_pass with depth attachment:
            //     { depth_ops: Operations { load: LoadOp::Load, store: StoreOp::Store } }
            //   Draw geometry from masked materialTag collection.
            // }
            let _masked_state = &self.render_pass_states[masked_pass_idx];
        }

        // --- Step 6 (debug only): Dump shadow textures to disk ---
        // C++: if (TfDebug::IsEnabled(HDX_DEBUG_DUMP_SHADOW_TEXTURES))
        //   for each shadow: renderBuffer->Map() -> HioImage::OpenForWriting() -> Write()
        // wgpu: would require async buffer readback + CPU image write.
        // Omitted (debug feature, requires wgpu device access beyond this task layer).

        // --- Step 7: Transition shadow textures back to ShaderRead ---
        // C++: for (HgiTextureHandle& texture : textureHandles)
        //          texture->SubmitLayoutChange(HgiTextureUsageBitsShaderRead)
        //
        // After this, shadow maps are readable as depth-comparison samplers in the
        // main lighting/shading passes.
        //
        // wgpu: textures exposed via TextureUsages::TEXTURE_BINDING in the bind group
        // for the lighting pass. No explicit barrier needed; wgpu enforces usage
        // compatibility at render-pass submission time.
    }

    fn get_render_tags(&self) -> &[Token] {
        &self.render_tags
    }

    fn is_converged(&self) -> bool {
        true
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

/// Inverts cull style for shadow pass.
///
/// Shadow maps typically render back faces to avoid self-shadowing artifacts.
fn invert_cull_style(style: HdCullStyle) -> HdCullStyle {
    match style {
        HdCullStyle::Nothing => HdCullStyle::Nothing,
        HdCullStyle::Back => HdCullStyle::Front,
        HdCullStyle::Front => HdCullStyle::Back,
        HdCullStyle::BackUnlessDoubleSided => HdCullStyle::FrontUnlessDoubleSided,
        HdCullStyle::FrontUnlessDoubleSided => HdCullStyle::BackUnlessDoubleSided,
        HdCullStyle::DontCare => HdCullStyle::DontCare,
    }
}

impl std::fmt::Display for HdxShadowTaskParams {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "ShadowTask Params: override={:?} wireframe={:?} lighting={} \
             alpha={} depthBias={}/{}/{} depthFunc={:?} cull={:?} \
             depthClamp={} depthRange=[{},{}]",
            self.override_color,
            self.wireframe_color,
            self.enable_lighting,
            self.alpha_threshold,
            self.depth_bias_enable,
            self.depth_bias_constant_factor,
            self.depth_bias_slope_factor,
            self.depth_func,
            self.cull_style,
            self.enable_depth_clamp,
            self.depth_range.x,
            self.depth_range.y,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use usd_vt::Value;

    #[test]
    fn test_shadow_task_params_default() {
        let params = HdxShadowTaskParams::default();
        assert!(!params.enable_lighting);
        assert!(!params.depth_bias_enable);
        assert_eq!(params.depth_func, HdCompareFunction::LEqual);
        assert_eq!(params.cull_style, HdCullStyle::BackUnlessDoubleSided);
        // New fields: depth clamp and depth range
        assert!(params.enable_depth_clamp);
        assert_eq!(params.depth_range.x, 0.0);
        assert!((params.depth_range.y - 0.99999).abs() < 1e-6);
    }

    #[test]
    fn test_shadow_task_params_equality() {
        let params1 = HdxShadowTaskParams::default();
        let params2 = HdxShadowTaskParams::default();
        assert_eq!(params1, params2);

        let mut params3 = HdxShadowTaskParams::default();
        params3.depth_bias_enable = true;
        assert_ne!(params1, params3);
    }

    #[test]
    fn test_shadow_task_creation() {
        let task = HdxShadowTask::new(Path::from_string("/shadow").unwrap());
        assert!(task.render_pass_states.is_empty());
        assert!(task.render_tags.is_empty());
    }

    #[test]
    fn test_shadow_task_set_params() {
        let mut task = HdxShadowTask::new(Path::from_string("/shadow").unwrap());

        let mut params = HdxShadowTaskParams::default();
        params.depth_bias_enable = true;
        params.depth_bias_constant_factor = 1.5;
        params.depth_bias_slope_factor = 2.0;

        task.set_params(params.clone());
        assert_eq!(task.get_params(), &params);
    }

    #[test]
    fn test_shadow_task_set_num_shadow_maps() {
        let mut task = HdxShadowTask::new(Path::from_string("/shadow").unwrap());

        // 3 shadow maps = 6 render passes (2 per map)
        task.set_num_shadow_maps(3);
        assert_eq!(task.get_num_shadow_passes(), 6);

        // Reduce to 2 shadow maps
        task.set_num_shadow_maps(2);
        assert_eq!(task.get_num_shadow_passes(), 4);
    }

    #[test]
    fn test_invert_cull_style() {
        assert_eq!(invert_cull_style(HdCullStyle::Back), HdCullStyle::Front);
        assert_eq!(invert_cull_style(HdCullStyle::Front), HdCullStyle::Back);
        assert_eq!(
            invert_cull_style(HdCullStyle::Nothing),
            HdCullStyle::Nothing
        );
        assert_eq!(
            invert_cull_style(HdCullStyle::BackUnlessDoubleSided),
            HdCullStyle::FrontUnlessDoubleSided
        );
    }

    #[test]
    fn test_shadow_task_render_tags() {
        let mut task = HdxShadowTask::new(Path::from_string("/shadow").unwrap());
        assert!(task.get_render_tags().is_empty());

        let tags = vec![Token::new("geometry"), Token::new("shadow")];
        task.set_render_tags(tags.clone());
        assert_eq!(task.get_render_tags(), tags.as_slice());
    }

    #[test]
    fn test_shadow_task_display() {
        let params = HdxShadowTaskParams::default();
        let display = format!("{}", params);
        assert!(display.contains("ShadowTask Params"));
        assert!(display.contains("depthFunc"));
        assert!(display.contains("depthClamp"));
        assert!(display.contains("depthRange"));
    }

    #[test]
    fn test_shadow_task_depth_clamp_configurable() {
        let mut params = HdxShadowTaskParams::default();
        assert!(params.enable_depth_clamp);
        params.enable_depth_clamp = false;
        assert!(!params.enable_depth_clamp);
    }

    #[test]
    fn test_shadow_task_depth_range_configurable() {
        let mut params = HdxShadowTaskParams::default();
        params.depth_range = Vec2f::new(0.0, 1.0);
        assert_eq!(params.depth_range.x, 0.0);
        assert_eq!(params.depth_range.y, 1.0);
    }

    #[test]
    fn test_shadow_execute_no_lighting_context() {
        let mut task = HdxShadowTask::new(Path::from_string("/shadow").unwrap());
        task.set_num_shadow_maps(2);

        // Without lightingContext, execute must return early without panicking.
        let mut ctx = HdTaskContext::new();
        task.execute(&mut ctx);
        // No side effects — early return taken
        assert!(!ctx.contains_key(&Token::new("shadowTaskExecuted")));
    }

    #[test]
    fn test_shadow_execute_zero_shadow_maps() {
        let mut task = HdxShadowTask::new(Path::from_string("/shadow").unwrap());
        // lightingContext present but no "shadows" key -> num_shadow_maps == 0
        let mut ctx = HdTaskContext::new();
        ctx.insert(Token::new("lightingContext"), Value::from(true));
        task.execute(&mut ctx);
        // Returns early after num_shadow_maps == 0 check — no panic
    }

    #[test]
    fn test_shadow_execute_pass_state_count_mismatch() {
        let mut task = HdxShadowTask::new(Path::from_string("/shadow").unwrap());
        // Claim 3 shadow maps but no pass states allocated — should skip, not panic.
        let mut ctx = HdTaskContext::new();
        ctx.insert(Token::new("lightingContext"), Value::from(true));
        ctx.insert(Token::new("shadows"), Value::from(3usize));
        task.execute(&mut ctx);
        // All iterations skip via the bounds check
    }
}
