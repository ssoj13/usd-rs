
//! HdStDrawTarget - Render-to-texture sprim.
//!
//! Represents an offscreen render target for render-to-texture passes.
//! See pxr/imaging/hdSt/drawTarget.h for C++ reference.

use std::sync::LazyLock;
use usd_gf::vec2::Vec2i;
use usd_hd::prim::{HdSceneDelegate, HdSprim};
use usd_hd::render::HdRprimCollection;
use usd_hd::types::HdDirtyBits;
use usd_sdf::Path as SdfPath;
use usd_tf::Token;

// Token keys matching C++ HdStDrawTargetTokens
static TOK_ENABLE: LazyLock<Token> = LazyLock::new(|| Token::new("enable"));
static TOK_CAMERA: LazyLock<Token> = LazyLock::new(|| Token::new("camera"));
static TOK_RESOLUTION: LazyLock<Token> = LazyLock::new(|| Token::new("resolution"));
static TOK_COLLECTION: LazyLock<Token> = LazyLock::new(|| Token::new("collection"));

/// Render pass state for a draw target.
///
/// Contains camera, collection, resolution, and AOV bindings.
#[derive(Debug, Clone)]
pub struct HdStDrawTargetRenderPassState {
    /// Camera path for this draw target
    pub camera_id: SdfPath,

    /// Rprim collection to render
    pub rprim_collection: HdRprimCollection,

    /// Resolution (width, height)
    pub resolution: (i32, i32),

    /// Whether draw target is enabled
    pub enabled: bool,
}

impl Default for HdStDrawTargetRenderPassState {
    fn default() -> Self {
        Self::new()
    }
}

impl HdStDrawTargetRenderPassState {
    /// Create default state.
    pub fn new() -> Self {
        Self {
            camera_id: SdfPath::default(),
            rprim_collection: HdRprimCollection::new(Token::new("geometry")),
            resolution: (512, 512),
            enabled: true,
        }
    }

    /// Get draw target render pass state (const).
    pub fn get_camera(&self) -> &SdfPath {
        &self.camera_id
    }

    /// Get resolution.
    pub fn get_resolution(&self) -> (i32, i32) {
        self.resolution
    }
}

/// Draw target (render-to-texture) state primitive.
#[derive(Debug)]
pub struct HdStDrawTarget {
    /// Prim path
    id: SdfPath,

    /// Dirty bits
    dirty_bits: HdDirtyBits,

    /// Whether enabled
    enabled: bool,

    /// Render pass state
    draw_target_render_pass_state: HdStDrawTargetRenderPassState,
}

impl HdStDrawTarget {
    /// Create a new draw target.
    pub fn new(id: SdfPath) -> Self {
        Self {
            id,
            dirty_bits: Self::ALL_DIRTY,
            enabled: true,
            draw_target_render_pass_state: HdStDrawTargetRenderPassState::new(),
        }
    }

    /// Whether the draw target is enabled.
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Get draw target render pass state.
    pub fn get_draw_target_render_pass_state(&self) -> &HdStDrawTargetRenderPassState {
        &self.draw_target_render_pass_state
    }
}

impl HdSprim for HdStDrawTarget {
    fn get_id(&self) -> &SdfPath {
        &self.id
    }

    fn get_dirty_bits(&self) -> HdDirtyBits {
        self.dirty_bits
    }

    fn set_dirty_bits(&mut self, bits: HdDirtyBits) {
        self.dirty_bits = bits;
    }

    fn sync(
        &mut self,
        delegate: &dyn HdSceneDelegate,
        _render_param: Option<&dyn usd_hd::prim::HdRenderParam>,
        dirty_bits: &mut HdDirtyBits,
    ) {
        let id = self.id.clone();
        let bits = *dirty_bits;

        // DirtyDTEnable: read enable flag (optional, defaults to true).
        if bits & Self::DIRTY_PARAMS != 0 {
            let val = delegate.get(&id, &TOK_ENABLE);
            self.enabled = val.get::<bool>().copied().unwrap_or(true);
            self.draw_target_render_pass_state.enabled = self.enabled;
        }

        // DirtyDTCamera: read camera SdfPath.
        if bits & Self::DIRTY_PARAMS != 0 {
            let val = delegate.get(&id, &TOK_CAMERA);
            if let Some(camera_path) = val.get::<SdfPath>() {
                self.draw_target_render_pass_state.camera_id = camera_path.clone();
            }
        }

        // DirtyDTResolution: read GfVec2i resolution.
        if bits & Self::DIRTY_PARAMS != 0 {
            let val = delegate.get(&id, &TOK_RESOLUTION);
            if let Some(res) = val.get::<Vec2i>() {
                self.draw_target_render_pass_state.resolution = (res[0], res[1]);
            }
        }

        // DirtyDTCollection: read rprim collection.
        if bits & Self::DIRTY_PARAMS != 0 {
            let val = delegate.get(&id, &TOK_COLLECTION);
            if let Some(collection) = val.get::<HdRprimCollection>() {
                self.draw_target_render_pass_state.rprim_collection = collection.clone();
            }
        }

        *dirty_bits = Self::CLEAN;
        self.dirty_bits = Self::CLEAN;
    }
}
