
//! Render task - Main geometry rendering task.
//!
//! Executes rendering for a collection of primitives using a render pass.
//! Port of pxr/imaging/hdx/renderTask.h/cpp

use usd_hd::prim::HdSceneDelegate;
use usd_hd::render::{HdRenderIndexTrait, HdTask, HdTaskContext, TfTokenVector};
use usd_sdf::Path;
use usd_tf::Token;
use usd_vt::Value;

use super::render_setup_task::{
    HdRenderPassAovBinding, HdxRenderPassState, HdxRenderPassStateHandle, HdxRenderSetupTask,
    HdxRenderTaskParams,
};

/// Backend execution request emitted by `HdxRenderTask::execute()`.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct HdxRenderTaskRequest {
    /// Material tag filter for this render pass.
    pub material_tag: Token,
    /// Render tags that scoped this task.
    pub render_tags: TfTokenVector,
    /// Render-pass state prepared for this task.
    pub render_pass_state: HdxRenderPassStateHandle,
}

/// Render task for drawing geometry.
///
/// A task for rendering geometry to pixels.
///
/// Rendering state management can be handled two ways:
/// 1. An application can create an HdxRenderTask and pass it the
///    HdxRenderTaskParams struct as "params".
/// 2. An application can create an HdxRenderSetupTask and an
///    HdxRenderTask, and pass params to the setup task. In this case
///    the setup task must run first.
///
/// Parameter unpacking is handled by HdxRenderSetupTask; in case #1,
/// HdxRenderTask creates a dummy setup task internally to manage the sync
/// process.
pub struct HdxRenderTask {
    /// Task path
    id: Path,

    /// Render tags for filtering
    render_tags: TfTokenVector,

    /// Material tag for collection filtering.
    material_tag: Token,

    /// Optional internal render setup task for params unpacking
    setup_task: Option<HdxRenderSetupTask>,

    /// Convergence state
    converged: bool,
}

impl HdxRenderTask {
    /// Create new render task.
    pub fn new(id: Path) -> Self {
        Self {
            id,
            render_tags: Vec::new(),
            material_tag: Token::default(),
            setup_task: None,
            converged: false,
        }
    }

    /// Set render tags for filtering.
    pub fn set_render_tags(&mut self, tags: TfTokenVector) {
        self.render_tags = tags;
    }

    /// Set material tag for collection filtering.
    pub fn set_material_tag(&mut self, material_tag: Token) {
        self.material_tag = material_tag;
    }

    /// Set render task parameters directly.
    pub fn set_params(&mut self, params: &HdxRenderTaskParams) {
        if self.setup_task.is_none() {
            self.setup_task = Some(HdxRenderSetupTask::new(self.id.clone()));
        }
        if let Some(setup) = &mut self.setup_task {
            setup.sync_params(params);
        }
    }

    /// Get current render task parameters (returns default if not yet set).
    pub fn get_params(&self) -> Option<HdxRenderTaskParams> {
        self.setup_task.as_ref().map(|s| s.get_params())
    }

    /// Set AOV bindings on the internal setup task.
    ///
    /// `bindings` are the primary AOV outputs (with or without clear values).
    /// `input_bindings` are used for depth compositing in volume passes.
    pub fn set_aov_bindings(
        &mut self,
        bindings: Vec<HdRenderPassAovBinding>,
        input_bindings: Vec<HdRenderPassAovBinding>,
    ) {
        if self.setup_task.is_none() {
            self.setup_task = Some(HdxRenderSetupTask::new(self.id.clone()));
        }
        if let Some(setup) = &mut self.setup_task {
            let mut params = setup.get_params();
            params.aov_bindings = bindings;
            params.aov_input_bindings = input_bindings;
            setup.sync_params(&params);
        }
    }

    /// Check if any AOVs need to be cleared.
    fn need_to_clear_aovs(&self, state: &HdxRenderPassState) -> bool {
        state
            .get_aov_bindings()
            .iter()
            .any(|binding| !binding.clear_value.is_empty())
    }

    /// Check if the render pass has draw items (Storm-specific).
    fn has_draw_items(&self) -> bool {
        // For non-Storm backends, always return true
        // Storm would check HdSt_RenderPass::HasDrawItems()
        true
    }
}

impl HdTask for HdxRenderTask {
    fn id(&self) -> &Path {
        &self.id
    }

    fn sync(
        &mut self,
        _delegate: &dyn HdSceneDelegate,
        _ctx: &mut HdTaskContext,
        dirty_bits: &mut u32,
    ) {
        // In a full implementation, we would pull params and collection from delegate
        // For now, params are set via set_params() directly

        // Mark dirty bits as clean
        *dirty_bits = 0;
    }

    fn prepare(&mut self, ctx: &mut HdTaskContext, render_index: &dyn HdRenderIndexTrait) {
        // Delegate to internal setup task if present
        if let Some(setup) = &mut self.setup_task {
            setup.prepare(ctx, render_index);
        }
    }

    fn execute(&mut self, ctx: &mut HdTaskContext) {
        // Retrieve render pass state from our internal setup task or the context
        // written by an external HdxRenderSetupTask.
        // C++: HdRenderPassStateSharedPtr renderPassState = _GetRenderPassState(ctx);
        let render_pass_state = if let Some(setup) = &self.setup_task {
            // Case 1: we own an internal setup task (params were set directly).
            Some(setup.get_render_pass_state())
        } else {
            // Case 2: external HdxRenderSetupTask ran first and placed state in context.
            ctx.get(&Token::new("renderPassState"))
                .and_then(|v| v.get::<HdxRenderPassStateHandle>())
                .map(|h| h.0.clone())
        };

        // C++: if (!TF_VERIFY(renderPassState)) return;
        let Some(state) = render_pass_state else {
            self.converged = true;
            return;
        };

        // C++: if (HdStRenderPassState* extendedState = dynamic_cast<...>(state.get())) {
        //        if (!_HasDrawItems() && !_NeedToClearAovs(state)) return;
        //        _SetHdStRenderPassState(ctx, extendedState);
        //      }
        //
        // The Storm-specific early-out and lighting/selection wiring are guarded
        // by the HdStRenderPassState downcast.  Non-Storm backends skip this block
        // entirely and always proceed to Execute().
        //
        // In our wgpu backend we emulate the Storm path:
        // - has_draw_items() is always true for non-Storm, so the early-out never
        //   fires unless AOVs also need no clearing.
        // - _SetHdStRenderPassState: bind lighting shader and selection BARs from ctx.
        {
            // Storm early-out: skip when nothing to draw and no AOV clears needed.
            if !self.has_draw_items() && !self.need_to_clear_aovs(&state) {
                self.converged = true;
                return;
            }

            // _SetHdStRenderPassState: wire lighting shader and selection buffers.
            // C++:
            //   lightingShader = ctx[HdxTokens->lightingShader]
            //   renderPassState->SetLightingShader(lightingShader)
            //   vo = ctx[HdxTokens->selectionOffsets]  (HdBufferArrayRange SSBO)
            //   vu = ctx[HdxTokens->selectionUniforms] (HdBufferArrayRange UBO)
            //   renderPassShader->AddBufferBinding / RemoveBufferBinding
            let has_lighting = ctx.contains_key(&Token::new("lightingShader"));
            let has_selection = ctx.contains_key(&Token::new("selectionOffsets"))
                && ctx.contains_key(&Token::new("selectionUniforms"));
            // wgpu: bind lighting UBO and selection SSBO+UBO to the render pipeline
            // when the corresponding tasks (SimpleLightTask, SelectionTask) have
            // populated those context keys.  Actual binding happens in the engine
            // render loop that drives wgpu RenderPass recording.
            let _ = (has_lighting, has_selection);
        }

        // C++: if (_pass) { _pass->Execute(renderPassState, GetRenderTags()); }
        //
        // wgpu equivalent (executed by engine/mod.rs render loop):
        //   1. Begin RenderPass with AOV textures as color+depth attachments,
        //      applying clear values from state.aov_bindings.
        //   2. Set viewport from state.framing / get_viewport().
        //   3. Set depth bias, stencil, blend from state.
        //   4. For each draw item filtered by GetRenderTags():
        //        bind vertex/index buffers, per-draw BARs, material pipeline,
        //        call draw_indexed(instance_count).
        //   5. End RenderPass, submit CommandBuffer.
        ctx.insert(Token::new("renderTaskRequested"), Value::from(true));
        let request = HdxRenderTaskRequest {
            material_tag: self.material_tag.clone(),
            render_tags: self.render_tags.clone(),
            render_pass_state: HdxRenderPassStateHandle::new(state.clone()),
        };
        let requests_token = Token::new("renderTaskRequests");
        if let Some(requests) = ctx
            .get_mut(&requests_token)
            .and_then(|value| value.get_mut::<Vec<HdxRenderTaskRequest>>())
        {
            requests.push(request);
        } else {
            ctx.insert(requests_token, Value::new(vec![request]));
        }

        self.converged = true;
    }

    fn get_render_tags(&self) -> &[Token] {
        &self.render_tags
    }

    fn is_converged(&self) -> bool {
        self.converged
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use usd_gf::Vec4d;

    #[test]
    fn test_render_task_creation() {
        let task = HdxRenderTask::new(Path::from_string("/render").unwrap());
        assert!(!task.is_converged());
        assert!(task.render_tags.is_empty());
        assert!(task.setup_task.is_none());
    }

    #[test]
    fn test_render_task_has_draw_items() {
        let task = HdxRenderTask::new(Path::from_string("/render").unwrap());
        // Non-Storm always returns true
        assert!(task.has_draw_items());
    }

    #[test]
    fn test_render_task_get_render_tags() {
        let mut task = HdxRenderTask::new(Path::from_string("/render").unwrap());
        task.render_tags = vec![Token::new("geometry"), Token::new("guide")];

        let tags = task.get_render_tags();
        assert_eq!(tags.len(), 2);
    }

    #[test]
    fn test_render_task_set_params() {
        let mut task = HdxRenderTask::new(Path::from_string("/render").unwrap());

        let mut params = HdxRenderTaskParams::default();
        params.enable_lighting = true;
        params.viewport = Vec4d::new(0.0, 0.0, 1920.0, 1080.0);

        task.set_params(&params);
        assert!(task.setup_task.is_some());
    }

    #[test]
    fn test_render_task_execute() {
        let mut task = HdxRenderTask::new(Path::from_string("/render").unwrap());
        let mut ctx = HdTaskContext::new();

        // Set params first
        let params = HdxRenderTaskParams::default();
        task.set_params(&params);

        task.execute(&mut ctx);
        assert!(task.is_converged());
    }
}
