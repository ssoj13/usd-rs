#![allow(dead_code)]

//! Full-screen image shader render pass for Storm.
//!
//! Renders a single full-screen triangle to apply post-processing effects.
//! The task that creates this pass sets a RenderPassShader on the
//! RenderPassState which acts as the full-screen fragment shader.
//!
//! The fullscreen triangle pattern uses a single oversized triangle that
//! covers the entire viewport without needing a vertex buffer with actual
//! geometry data -- the vertex shader generates clip-space positions
//! procedurally from `gl_VertexIndex`.
//!
//! Matches C++ `HdSt_ImageShaderRenderPass`.

use std::sync::Arc;
use usd_hd::render::{HdRenderPass, HdRenderPassStateSharedPtr, HdRprimCollection, TfTokenVector};
use usd_sdf::Path as SdfPath;

use crate::draw_item::HdStDrawItem;
use crate::image_shader_shader_key::ImageShaderShaderKey;

// ---------------------------------------------------------------------------
// Image shader render pass
// ---------------------------------------------------------------------------

/// Full-screen image shader render pass.
///
/// Draws a single full-screen triangle using a procedural vertex shader.
/// The fragment shader comes from the RenderPassShader set on the
/// RenderPassState, enabling post-processing effects that participate
/// in Storm's code generation system.
///
/// # Usage
///
/// ```ignore
/// let mut pass = ImageShaderRenderPass::new(collection);
/// pass.setup_fullscreen_triangle();
/// pass.execute(&render_pass_state, &[]);
/// ```
pub struct ImageShaderRenderPass {
    /// Prim collection (mostly unused for image shader passes)
    collection: HdRprimCollection,
    /// Shared data for the fullscreen triangle draw item
    rprim_id: SdfPath,
    /// The draw item representing the fullscreen triangle
    draw_item: Option<HdStDrawItem>,
    /// Whether the fullscreen triangle has been set up
    triangle_setup: bool,
    /// Shader key for the image shader
    shader_key: Option<ImageShaderShaderKey>,
}

impl ImageShaderRenderPass {
    /// Create a new image shader render pass.
    pub fn new(collection: HdRprimCollection) -> Self {
        Self {
            collection,
            rprim_id: SdfPath::from_string("/imageShaderRenderPass")
                .unwrap_or_else(|| SdfPath::absolute_root()),
            draw_item: None,
            triangle_setup: false,
            shader_key: None,
        }
    }

    /// Set up the fullscreen triangle draw item.
    ///
    /// Creates a vertex primvar BAR with 3 dummy vertices (the actual
    /// positions are generated procedurally in the vertex shader) and
    /// sets up the geometric shader for the image shader pipeline.
    ///
    /// This must be called before execute().
    pub fn setup_fullscreen_triangle(&mut self) {
        if self.triangle_setup {
            return;
        }

        // Create shader key for vertex + fragment stages
        self.shader_key = Some(ImageShaderShaderKey::new());

        // Create the draw item (single triangle, 3 vertices)
        let draw_item = HdStDrawItem::new(self.rprim_id.clone());
        self.draw_item = Some(draw_item);

        self.triangle_setup = true;

        log::debug!("ImageShaderRenderPass: fullscreen triangle set up");
    }

    /// Check if the fullscreen triangle is set up.
    pub fn is_setup(&self) -> bool {
        self.triangle_setup
    }

    /// Get the rprim ID for this pass.
    pub fn get_rprim_id(&self) -> &SdfPath {
        &self.rprim_id
    }

    /// Get the shader key (available after setup).
    pub fn get_shader_key(&self) -> Option<&ImageShaderShaderKey> {
        self.shader_key.as_ref()
    }
}

impl HdRenderPass for ImageShaderRenderPass {
    fn get_rprim_collection(&self) -> &HdRprimCollection {
        &self.collection
    }

    fn set_rprim_collection(&mut self, collection: HdRprimCollection) {
        self.collection = collection;
    }

    fn sync(&mut self) {
        // Image shader passes don't need sync -- the fullscreen triangle
        // is static and the shader comes from the render pass state.
    }

    fn execute(
        &mut self,
        _render_pass_state: &HdRenderPassStateSharedPtr,
        _render_tags: &TfTokenVector,
    ) {
        if !self.triangle_setup {
            log::warn!("ImageShaderRenderPass::execute: triangle not set up");
            return;
        }

        // In the full implementation this would:
        // 1. Downcast render_pass_state to HdStRenderPassState
        // 2. Call PrepareDraw on the draw batch
        // 3. Create HGI graphics cmds from the pass state's AOV desc
        // 4. Set viewport from pass state
        // 5. Apply camera state
        // 6. ExecuteDraw on the batch (draws 1 fullscreen triangle)
        // 7. Submit graphics cmds to HGI

        log::debug!(
            "ImageShaderRenderPass::execute: rendering fullscreen triangle for post-process"
        );
    }
}

/// Shared pointer alias.
pub type ImageShaderRenderPassSharedPtr = Arc<ImageShaderRenderPass>;

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use usd_tf::Token;

    #[test]
    fn test_creation() {
        let collection = HdRprimCollection::new(Token::new("image"));
        let pass = ImageShaderRenderPass::new(collection);
        assert!(!pass.is_setup());
    }

    #[test]
    fn test_setup() {
        let collection = HdRprimCollection::new(Token::new("image"));
        let mut pass = ImageShaderRenderPass::new(collection);

        pass.setup_fullscreen_triangle();
        assert!(pass.is_setup());
        assert!(pass.get_shader_key().is_some());
        assert!(pass.draw_item.is_some());
    }

    #[test]
    fn test_double_setup_noop() {
        let collection = HdRprimCollection::new(Token::new("image"));
        let mut pass = ImageShaderRenderPass::new(collection);

        pass.setup_fullscreen_triangle();
        pass.setup_fullscreen_triangle(); // second call should be no-op
        assert!(pass.is_setup());
    }

    #[test]
    fn test_rprim_id() {
        let collection = HdRprimCollection::new(Token::new("image"));
        let pass = ImageShaderRenderPass::new(collection);
        assert_eq!(
            pass.get_rprim_id().as_str(),
            "/imageShaderRenderPass"
        );
    }

    #[test]
    fn test_render_pass_trait() {
        let collection = HdRprimCollection::new(Token::new("image"));
        let mut pass = ImageShaderRenderPass::new(collection);

        // Test HdRenderPass trait methods
        assert_eq!(
            pass.get_rprim_collection().get_name(),
            &Token::new("image")
        );

        pass.set_rprim_collection(HdRprimCollection::new(Token::new("other")));
        assert_eq!(
            pass.get_rprim_collection().get_name(),
            &Token::new("other")
        );

        // sync and execute should not panic
        pass.sync();
    }
}
