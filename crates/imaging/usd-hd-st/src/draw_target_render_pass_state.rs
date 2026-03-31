
//! Draw target render pass state (ported from drawTargetRenderPassState.h).
//!
//! Non-GL-specific render pass state for draw targets. Stores camera,
//! collection, AOV bindings, and depth priority for render-to-texture passes.

use usd_hd::render::HdRprimCollection;
use usd_sdf::Path as SdfPath;
use usd_tf::Token;

/// Depth priority for draw target rendering.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum HdDepthPriority {
    /// Render at default depth
    #[default]
    Default,
    /// Render nearer (overlay-like)
    Nearer,
    /// Render farther
    Farther,
}

/// AOV (Arbitrary Output Variable) binding for render passes.
#[derive(Debug, Clone)]
pub struct HdRenderPassAovBinding {
    /// AOV name (e.g. "color", "depth", "primId")
    pub aov_name: Token,
    /// Render buffer path
    pub render_buffer_path: SdfPath,
    /// Clear value (RGBA for color, depth value for depth)
    pub clear_value: [f32; 4],
}

/// Render pass state for draw targets (ported from drawTargetRenderPassState.h).
///
/// Contains all non-GL state needed to render into a draw target:
/// camera, collection, AOV bindings, depth priority.
#[derive(Debug)]
pub struct HdStDrawTargetRenderPassState {
    /// AOV bindings (color, depth, etc.)
    aov_bindings: Vec<HdRenderPassAovBinding>,
    /// Depth priority (closer/farther wins)
    depth_priority: HdDepthPriority,
    /// Camera to render from
    camera_id: SdfPath,
    /// Rprim collection to render
    rprim_collection: HdRprimCollection,
    /// Version counter for collection changes
    rprim_collection_version: u32,
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
            aov_bindings: Vec::new(),
            depth_priority: HdDepthPriority::Default,
            camera_id: SdfPath::default(),
            rprim_collection: HdRprimCollection::new(Token::new("geometry")),
            rprim_collection_version: 0,
        }
    }

    /// Get current AOV bindings.
    pub fn get_aov_bindings(&self) -> &[HdRenderPassAovBinding] {
        &self.aov_bindings
    }

    /// Set AOV bindings.
    pub fn set_aov_bindings(&mut self, bindings: Vec<HdRenderPassAovBinding>) {
        self.aov_bindings = bindings;
    }

    /// Set depth priority.
    pub fn set_depth_priority(&mut self, priority: HdDepthPriority) {
        self.depth_priority = priority;
    }

    /// Get depth priority.
    pub fn get_depth_priority(&self) -> HdDepthPriority {
        self.depth_priority
    }

    /// Set the camera path to render from.
    pub fn set_camera(&mut self, camera_id: SdfPath) {
        self.camera_id = camera_id;
    }

    /// Get the camera path.
    pub fn get_camera(&self) -> &SdfPath {
        &self.camera_id
    }

    /// Set the rprim collection.
    pub fn set_rprim_collection(&mut self, collection: HdRprimCollection) {
        self.rprim_collection = collection;
        self.rprim_collection_version += 1;
    }

    /// Get the rprim collection.
    pub fn get_rprim_collection(&self) -> &HdRprimCollection {
        &self.rprim_collection
    }

    /// Get the rprim collection version (increments on each change).
    pub fn get_rprim_collection_version(&self) -> u32 {
        self.rprim_collection_version
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_state() {
        let state = HdStDrawTargetRenderPassState::new();
        assert!(state.get_aov_bindings().is_empty());
        assert_eq!(state.get_depth_priority(), HdDepthPriority::Default);
        assert_eq!(state.get_rprim_collection_version(), 0);
    }

    #[test]
    fn test_set_collection_bumps_version() {
        let mut state = HdStDrawTargetRenderPassState::new();
        assert_eq!(state.get_rprim_collection_version(), 0);

        state.set_rprim_collection(HdRprimCollection::new(Token::new("shadow")));
        assert_eq!(state.get_rprim_collection_version(), 1);

        state.set_rprim_collection(HdRprimCollection::new(Token::new("shadow2")));
        assert_eq!(state.get_rprim_collection_version(), 2);
    }

    #[test]
    fn test_aov_bindings() {
        let mut state = HdStDrawTargetRenderPassState::new();
        let bindings = vec![
            HdRenderPassAovBinding {
                aov_name: Token::new("color"),
                render_buffer_path: SdfPath::from_string("/renderBuffers/color").unwrap(),
                clear_value: [0.0, 0.0, 0.0, 1.0],
            },
            HdRenderPassAovBinding {
                aov_name: Token::new("depth"),
                render_buffer_path: SdfPath::from_string("/renderBuffers/depth").unwrap(),
                clear_value: [1.0, 0.0, 0.0, 0.0],
            },
        ];
        state.set_aov_bindings(bindings);
        assert_eq!(state.get_aov_bindings().len(), 2);
    }
}
