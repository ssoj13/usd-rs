//! Skydome task - Render environment skydome/skybox.
//!
//! Renders a skydome or skybox background for the scene.
//! Port of pxr/imaging/hdx/skydomeTask.h/cpp

use usd_gf::Matrix4d;
use usd_hd::prim::HdSceneDelegate;
use usd_hd::render::{HdRenderIndexTrait, HdTask, HdTaskContext, TfTokenVector};
use usd_sdf::{AssetPath, Path};
use usd_tf::Token;
use usd_vt::Value;

/// Skydome task tokens.
pub mod skydome_tokens {
    use std::sync::LazyLock;
    use usd_tf::Token;

    /// Skydome texture file path.
    pub static TEXTURE_FILE: LazyLock<Token> = LazyLock::new(|| Token::new("texture:file"));

    /// Skydome rotation transform.
    pub static SKYDOME_TRANSFORM: LazyLock<Token> =
        LazyLock::new(|| Token::new("skydomeTransform"));

    /// Skydome light path (for dome light).
    pub static DOME_LIGHT: LazyLock<Token> = LazyLock::new(|| Token::new("domeLight"));
}

/// Skydome projection mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkydomeMode {
    /// Latitude-longitude spherical projection.
    LatLong,

    /// Cube map projection (6 faces).
    CubeMap,

    /// Angular (fisheye) projection.
    Angular,

    /// Mirror ball projection.
    MirrorBall,
}

impl Default for SkydomeMode {
    fn default() -> Self {
        Self::LatLong
    }
}

/// Skydome task parameters.
///
/// Configures skydome/skybox rendering.
/// Port of HdxSkydomeTaskParams from pxr/imaging/hdx/skydomeTask.h
#[derive(Debug, Clone)]
pub struct HdxSkydomeTaskParams {
    /// Path to skydome texture file (HDR or LDR).
    pub texture_file: AssetPath,

    /// Projection mode for texture mapping.
    pub mode: SkydomeMode,

    /// Additional transform for rotating skydome.
    pub transform: Matrix4d,

    /// Path to dome light prim (if using scene light).
    pub dome_light_path: Path,

    /// Exposure adjustment for HDR textures.
    pub exposure: f32,

    /// Render skydome (false = just clear to color).
    pub enable: bool,
}

impl Default for HdxSkydomeTaskParams {
    fn default() -> Self {
        Self {
            texture_file: AssetPath::default(),
            mode: SkydomeMode::LatLong,
            transform: Matrix4d::identity(),
            dome_light_path: Path::empty(),
            exposure: 0.0,
            enable: true,
        }
    }
}

/// Parameter buffer for the skydome shader.
///
/// Mirrors C++ HdxSkydomeTask::_ParameterBuffer:
///   struct _ParameterBuffer { GfMatrix4f invProjMatrix; GfMatrix4f viewToWorldMatrix; GfMatrix4f lightTransform; };
/// Initialized to identity matrices.
#[derive(Debug, Clone, PartialEq)]
struct SkydomeParameterBuffer {
    inv_proj_matrix: Matrix4d,
    view_to_world_matrix: Matrix4d,
    light_transform: Matrix4d,
}

impl Default for SkydomeParameterBuffer {
    fn default() -> Self {
        Self {
            inv_proj_matrix: Matrix4d::identity(),
            view_to_world_matrix: Matrix4d::identity(),
            light_transform: Matrix4d::identity(),
        }
    }
}

/// Skydome rendering task.
///
/// A task for rendering an environment skydome or skybox as the scene
/// background. Supports various projection modes and HDR/LDR textures.
///
/// The skydome is rendered at infinite distance (depth = 1.0) after
/// clearing but before scene geometry. It can be sourced from:
/// - Texture file (HDR or LDR)
/// - Dome light in the scene
/// - Procedural sky model
///
/// Port of HdxSkydomeTask from pxr/imaging/hdx/skydomeTask.h
pub struct HdxSkydomeTask {
    /// Task path.
    id: Path,

    /// Task parameters.
    params: HdxSkydomeTaskParams,

    /// Render tags for filtering.
    render_tags: TfTokenVector,

    /// Whether texture is loaded.
    texture_loaded: bool,

    /// Cached texture generation (for change tracking).
    texture_generation: u64,

    /// Whether skydome is visible by camera.
    /// C++: _skydomeVisibility -- read from render delegate settings
    ///   (HdRenderSettingsTokens->domeLightCameraVisibility, default true).
    skydome_visibility: bool,

    /// Whether the dome light cubemap texture is available.
    /// C++: set by _GetSkydomeTexture(), which queries HdStSimpleLightingShader
    ///   for the dome light cubemap texture and sampler handles.
    have_skydome_texture: bool,

    /// Cached parameter buffer for change detection.
    /// C++: _parameterData -- updated only when matrices change (avoids redundant GPU uploads).
    param_buffer: SkydomeParameterBuffer,
}

impl HdxSkydomeTask {
    /// Create new skydome task.
    pub fn new(id: Path) -> Self {
        Self {
            id,
            params: HdxSkydomeTaskParams::default(),
            render_tags: Vec::new(),
            texture_loaded: false,
            skydome_visibility: true,
            texture_generation: 0,
            have_skydome_texture: false,
            param_buffer: SkydomeParameterBuffer::default(),
        }
    }

    /// Set skydome parameters.
    pub fn set_params(&mut self, params: HdxSkydomeTaskParams) {
        self.params = params;
        self.texture_loaded = false; // Force reload
    }

    /// Get skydome parameters.
    pub fn get_params(&self) -> &HdxSkydomeTaskParams {
        &self.params
    }

    /// Check if texture is loaded.
    pub fn is_texture_loaded(&self) -> bool {
        self.texture_loaded
    }
}

impl HdTask for HdxSkydomeTask {
    fn id(&self) -> &Path {
        &self.id
    }

    fn sync(
        &mut self,
        _delegate: &dyn HdSceneDelegate,
        _ctx: &mut HdTaskContext,
        dirty_bits: &mut u32,
    ) {
        // In full implementation: pull params from scene delegate
        // For now, params are set via set_params() directly

        *dirty_bits = 0;
    }

    fn prepare(&mut self, ctx: &mut HdTaskContext, _render_index: &dyn HdRenderIndexTrait) {
        if !self.params.enable {
            return;
        }

        // In full implementation:
        // 1. Check if texture file changed
        // 2. Load texture from file or dome light
        // 3. Convert to GPU texture (handle HDR formats)
        // 4. Create/update skydome geometry (sphere or cube)
        // 5. Prepare skydome shader with projection mode
        // 6. Set up render state (no depth write, depth = 1.0)

        // Simulate texture loading
        if !self.params.texture_file.get_asset_path().is_empty() {
            self.texture_loaded = true;
            self.texture_generation += 1;
        }

        ctx.insert(Token::new("skydomePrepared"), Value::from(true));
    }

    fn execute(&mut self, ctx: &mut HdTaskContext) {
        // C++: HdxSkydomeTask::Execute() -- pxr/imaging/hdx/skydomeTask.cpp

        // --- Step 1: Get HdStRenderPassState and build HgiGraphicsCmdsDesc ---
        // C++: HdRenderPassStateSharedPtr renderPassState = _GetRenderPassState(ctx)
        //      _GetRenderPassState checks _setupTask first (when HdxRenderTaskParams was
        //      provided at Sync time), then falls back to ctx[HdxTokens->renderPassState].
        //      HdStRenderPassState* hdStRenderPassState = dynamic_cast<...>(renderPassState)
        //      if (!hdStRenderPassState) return;
        //      HgiGraphicsCmdsDesc gfxCmdsDesc = hdStRenderPassState->MakeGraphicsCmdsDesc()
        //
        // In the Rust port we check the context key that render_setup_task publishes.
        // If not present, we cannot determine AOV targets -- return early.
        let rps_token = Token::new("renderPassState");
        if !ctx.contains_key(&rps_token) {
            return;
        }

        // --- Step 2: Check if color AOV is present ---
        // C++: const bool haveColorAOV = !gfxCmdsDesc.colorTextures.empty();
        //
        // gfxCmdsDesc.colorTextures is populated from the HdStRenderPassState AOV bindings.
        // Without a color texture we cannot render the skydome (no render target to draw into).
        //
        // In the Rust port, the presence of the "color" AOV in the task context serves
        // as the equivalent signal.
        let have_color_aov = ctx.contains_key(&Token::new("color"));

        // --- Step 3: Determine whether AOVs need to be cleared ---
        // C++:
        //   const bool needClear =
        //     (gfxCmdsDesc.depthAttachmentDesc.loadOp == HgiAttachmentLoadOpClear) ||
        //     (!gfxCmdsDesc.colorAttachmentDescs.empty() &&
        //      gfxCmdsDesc.colorAttachmentDescs[0].loadOp == HgiAttachmentLoadOpClear);
        //
        // needClear is true when the render pass state specifies clear on load --
        // even if the skydome itself cannot render, the AOV must still be cleared.
        //
        // In the Rust port we read this from a context key set by HdxRenderSetupTask
        // ("aovNeedsClear"). If absent, conservatively assume no clear needed.
        let need_clear = ctx
            .get(&Token::new("aovNeedsClear"))
            .and_then(|v| v.get::<bool>().copied())
            .unwrap_or(false);

        // --- Step 4: Find dome light and its inverse transform ---
        // C++: bool haveDomeLight = false; GfMatrix4f lightTransform(1);
        //      if (_skydomeVisibility)
        //        if (_GetTaskContextData(ctx, HdxTokens->lightingContext, &lightingContext))
        //          for (int i = 0; i < lightingContext->GetNumLightsUsed(); ++i)
        //            if (lights[i].IsDomeLight())
        //              lightTransform = GfMatrix4f(light.GetTransform().GetInverse())
        //              haveDomeLight = true; break;
        let mut have_dome_light = false;
        let mut light_transform = Matrix4d::identity();
        if self.skydome_visibility {
            if ctx.contains_key(&Token::new("lightingContext")) {
                // In full Storm impl: extract GlfSimpleLightingContext, iterate GetLights(),
                // find IsDomeLight(), compute GetTransform().GetInverse() -> light_transform.
                // Here we mark the dome light present if the lighting context is available;
                // the actual transform is extracted by the engine layer before Execute().
                if let Some(v) = ctx.get(&Token::new("domeLightTransformInv")) {
                    if let Some(m) = v.get::<Matrix4d>() {
                        light_transform = *m;
                    }
                    have_dome_light = true;
                } else {
                    // Lighting context present but no dome light was found.
                    have_dome_light = false;
                }
            }
        }

        // --- Step 5: Get skydome texture from HdStSimpleLightingShader ---
        // C++: bool textureAvailable = _GetSkydomeTexture(ctx)
        //   _GetSkydomeTexture: extracts lightingShader from ctx, casts to
        //   HdStSimpleLightingShader, gets dome light cubemap texture handle,
        //   validates HdStCubemapTextureObject and HdStCubemapSamplerObject.
        //   Stores _skydomeTexture and _skydomeSampler for the draw call.
        //
        // In the Rust port, the texture availability is tracked by have_skydome_texture
        // (updated in prepare()). A dedicated context key "skydomeTexture" would carry
        // the wgpu texture view; here we use the flag as the availability gate.
        let have_texture =
            self.have_skydome_texture || ctx.contains_key(&Token::new("skydomeTexture"));

        // --- Step 6: Early-out with optional clear ---
        // C++: if (!_skydomeVisibility || !haveColorAOV || !haveDomeLight || !textureAvailable)
        //   if (needClear)
        //     _GetHgi()->SubmitCmds(_GetHgi()->CreateGraphicsCmds(gfxCmdsDesc).get())
        //   return;
        //
        // An empty graphics command submission with the original gfxCmdsDesc ensures
        // the AOV attachments are cleared even when the skydome itself cannot render.
        if !self.skydome_visibility || !have_color_aov || !have_dome_light || !have_texture {
            if need_clear {
                // wgpu: Submit a render pass with LoadOp::Clear and no draw calls.
                // This is what C++ does: CreateGraphicsCmds(gfxCmdsDesc) with clear loadOps
                // set in the attachment descriptors -- the pass itself is empty.
                // Concrete wgpu implementation lives in the engine layer.
                ctx.insert(Token::new("skydomeNeedsClear"), Value::from(true));
            }
            return;
        }

        // --- Step 7: Set fragment shader for fullscreen compositor ---
        // C++: _SetFragmentShader()
        //   Configures HdxFullscreenShader with HgiShaderFunctionDesc:
        //     - input: uvOut (vec2)
        //     - texture: skydomeTexture (cubemap, binding 0, HgiFormatFloat16Vec4)
        //     - output: hd_FragColor (vec4), gl_FragDepth (float, depth(any))
        //     - push constants: invProjMatrix (mat4), viewToWorld (mat4), lightTransform (mat4)
        //   Calls _compositor->SetProgram(HdxPackageSkydomeShader(), skydomeFrag, fragDesc)
        //
        // wgpu: The skydome WGSL shader is compiled once and cached in the engine.
        // The fragment shader samples the cubemap using:
        //   let worldDir = (viewToWorld * (invProj * vec4(uv * 2 - 1, 1, 1))).xyz;
        //   let envColor = textureSample(skydomeTexture, skydomeSampler, lightTransform * worldDir);
        //   hd_FragColor = envColor;
        //   gl_FragDepth = 1.0;  // skydome renders at infinite distance

        // --- Step 8: Compute matrices and update parameter buffer if changed ---
        // C++:
        //   const GfMatrix4f invProjMatrix(hdStRenderPassState->GetProjectionMatrix().GetInverse())
        //   const GfMatrix4f viewToWorldMatrix(hdStRenderPassState->GetWorldToViewMatrix().GetInverse())
        //   if (_UpdateParameterBuffer(invProjMatrix, viewToWorldMatrix, lightTransform))
        //     _compositor->SetShaderConstants(sizeof(_ParameterBuffer), &_parameterData)
        //
        // _UpdateParameterBuffer returns true only when any matrix has changed
        // (avoids redundant GPU buffer uploads).
        //
        // In the Rust port we read the matrices from the render pass state context keys
        // published by the camera/setup task. The engine layer reads view/proj from
        // HdxRenderPassState and stores their inverses here.
        let inv_proj = ctx
            .get(&Token::new("invProjMatrix"))
            .and_then(|v| v.get::<Matrix4d>().copied())
            .unwrap_or(Matrix4d::identity());
        let view_to_world = ctx
            .get(&Token::new("viewToWorldMatrix"))
            .and_then(|v| v.get::<Matrix4d>().copied())
            .unwrap_or(Matrix4d::identity());

        // Change-detect: only upload constants if any matrix changed.
        // C++: if (_UpdateParameterBuffer(invProj, viewToWorld, lightTransform)) ...
        let new_buf = SkydomeParameterBuffer {
            inv_proj_matrix: inv_proj,
            view_to_world_matrix: view_to_world,
            light_transform,
        };
        let params_dirty = new_buf != self.param_buffer;
        if params_dirty {
            self.param_buffer = new_buf;
            // wgpu: write_buffer(param_buffer_gpu, 0, bytemuck::cast_slice(&[self.param_buffer]))
            // C++: _compositor->SetShaderConstants(sizeof(_ParameterBuffer), &_parameterData)
        }

        // --- Step 9: Bind skydome texture and sampler ---
        // C++: _compositor->BindTextures({_skydomeTexture}, {_skydomeSampler})
        //   _skydomeTexture  -- HgiTextureHandle to the dome light cubemap (float16 RGBA)
        //   _skydomeSampler  -- HgiSamplerHandle (linear, clamp-to-edge)
        //
        // wgpu: bind_group with:
        //   binding 0: texture_view (cubemap, TEXTURE_BINDING usage)
        //   binding 1: sampler (FilterMode::Linear, AddressMode::ClampToEdge)

        // --- Step 10: Get viewport ---
        // C++: GfVec4i viewport = hdStRenderPassState->ComputeViewport()
        //   Returns (x, y, width, height) from framing or legacy viewport field.
        //
        // wgpu: the render pass encoder sets viewport via set_viewport().

        // --- Step 11: Draw fullscreen quad via compositor ---
        // C++:
        //   HgiTextureHandle colorDst       = gfxCmdsDesc.colorTextures.empty() ? {} : colorTextures[0]
        //   HgiTextureHandle colorResolveDst = gfxCmdsDesc.colorResolveTextures.empty() ? {} : ...[0]
        //   HgiTextureHandle depthDst        = gfxCmdsDesc.depthTexture
        //   HgiTextureHandle depthResolveDst = gfxCmdsDesc.depthResolveTexture
        //   if (needClear) _compositor->SetClearState(clearColor, clearDepth)
        //   _compositor->Draw(colorDst, colorResolveDst, depthDst, depthResolveDst, viewport)
        //
        // HdxFullscreenShader::Draw() internally:
        //   1. Creates HgiGraphicsPipeline (skydome VS+FS, no depth write, depth test = always)
        //   2. Creates HgiGraphicsCmds with the AOV textures as color/depth attachments
        //   3. Binds pipeline, resources, vertex buffer (fullscreen triangle [-1,3])
        //   4. Calls DrawIndexed(3 verts) to shade every pixel
        //   5. Submits commands via hgi->SubmitCmds()
        //
        // wgpu equivalent:
        //   let mut pass = encoder.begin_render_pass(&RenderPassDescriptor {
        //     color_attachments: [Some(RenderPassColorAttachment {
        //       view: &color_dst_view,
        //       ops: Operations { load: if need_clear { LoadOp::Clear(clear_color) }
        //                                         else { LoadOp::Load },
        //                         store: StoreOp::Store },
        //     })],
        //     depth_stencil_attachment: Some(RenderPassDepthStencilAttachment {
        //       view: &depth_dst_view,
        //       depth_ops: Some(Operations { load: LoadOp::Load, store: StoreOp::Discard }),
        //     }),
        //   });
        //   pass.set_pipeline(&skydome_pipeline);  // depth_write_enabled=false, depth_compare=Always
        //   pass.set_bind_group(0, &skydome_bind_group, &[]);
        //   pass.draw(0..3, 0..1);  // fullscreen triangle

        // Record that backend execution should perform the skydome pass.
        ctx.insert(Token::new("skydomeRenderRequested"), Value::from(true));
        // Preserve the legacy flag used by existing tests.
        ctx.insert(Token::new("skydomeTaskExecuted"), Value::from(true));
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_skydome_tokens() {
        use skydome_tokens::*;
        assert_eq!(TEXTURE_FILE.as_str(), "texture:file");
        assert_eq!(SKYDOME_TRANSFORM.as_str(), "skydomeTransform");
        assert_eq!(DOME_LIGHT.as_str(), "domeLight");
    }

    #[test]
    fn test_skydome_mode() {
        assert_eq!(SkydomeMode::default(), SkydomeMode::LatLong);
        assert_ne!(SkydomeMode::LatLong, SkydomeMode::CubeMap);
        assert_ne!(SkydomeMode::Angular, SkydomeMode::MirrorBall);
    }

    #[test]
    fn test_skydome_params_default() {
        let params = HdxSkydomeTaskParams::default();
        assert_eq!(params.mode, SkydomeMode::LatLong);
        assert_eq!(params.exposure, 0.0);
        assert!(params.enable);
        assert!(params.dome_light_path.is_empty());
    }

    #[test]
    fn test_skydome_task_creation() {
        let task = HdxSkydomeTask::new(Path::from_string("/skydome").unwrap());
        assert!(!task.is_texture_loaded());
        assert!(task.render_tags.is_empty());
        assert_eq!(task.texture_generation, 0);
    }

    #[test]
    fn test_skydome_task_set_params() {
        let mut task = HdxSkydomeTask::new(Path::from_string("/skydome").unwrap());

        let mut params = HdxSkydomeTaskParams::default();
        params.texture_file = AssetPath::new("sky.hdr");
        params.mode = SkydomeMode::CubeMap;
        params.exposure = 1.5;
        params.enable = false;

        task.set_params(params.clone());
        assert_eq!(task.get_params().mode, SkydomeMode::CubeMap);
        assert_eq!(task.get_params().exposure, 1.5);
        assert!(!task.get_params().enable);
        assert!(!task.is_texture_loaded());
    }

    #[test]
    fn test_skydome_task_texture_state() {
        let mut task = HdxSkydomeTask::new(Path::from_string("/skydome").unwrap());

        // Initially no texture
        assert!(!task.is_texture_loaded());

        // Setting params resets texture state
        let mut params = HdxSkydomeTaskParams::default();
        params.texture_file = AssetPath::new("environment.hdr");
        task.set_params(params);
        assert!(!task.is_texture_loaded());

        // Simulate texture loading
        task.texture_loaded = true;
        assert!(task.is_texture_loaded());
    }

    #[test]
    fn test_skydome_task_execute() {
        let mut task = HdxSkydomeTask::new(Path::from_string("/skydome").unwrap());
        let mut ctx = HdTaskContext::new();

        // Without renderPassState in context -- return at step 1, nothing inserted.
        task.execute(&mut ctx);
        assert!(!ctx.contains_key(&Token::new("skydomeTaskExecuted")));

        // With renderPassState but no color AOV, no dome light, no texture --
        // early-out at step 6 (skydome conditions not met).
        ctx.insert(Token::new("renderPassState"), Value::from(true));
        task.execute(&mut ctx);
        assert!(!ctx.contains_key(&Token::new("skydomeTaskExecuted")));

        // Full success path: renderPassState + color AOV + lightingContext +
        // domeLightTransformInv (dome light found) + skydomeTexture (cubemap ready).
        // C++ equivalent: hdStRenderPassState valid, haveColorAOV, haveDomeLight,
        //                 _GetSkydomeTexture() returns true.
        task.skydome_visibility = true;
        ctx.insert(Token::new("color"), Value::from(true));
        ctx.insert(Token::new("lightingContext"), Value::from(true));
        ctx.insert(
            Token::new("domeLightTransformInv"),
            Value::from(Matrix4d::identity()),
        );
        ctx.insert(Token::new("skydomeTexture"), Value::from(true));
        task.execute(&mut ctx);
        assert!(ctx.contains_key(&Token::new("skydomeTaskExecuted")));
    }

    #[test]
    fn test_skydome_task_execute_need_clear_early_out() {
        // Verify that when conditions are not met but needClear is set,
        // the task inserts "skydomeNeedsClear" before returning.
        // C++: if (!_skydomeVisibility || !haveColorAOV || ...) { if (needClear) SubmitCmds(...); }
        let mut task = HdxSkydomeTask::new(Path::from_string("/skydome").unwrap());
        let mut ctx = HdTaskContext::new();
        ctx.insert(Token::new("renderPassState"), Value::from(true));
        ctx.insert(Token::new("aovNeedsClear"), Value::from(true));
        // No dome light / texture -- triggers early-out path.
        task.execute(&mut ctx);
        assert!(ctx.contains_key(&Token::new("skydomeNeedsClear")));
        assert!(!ctx.contains_key(&Token::new("skydomeTaskExecuted")));
    }
}
