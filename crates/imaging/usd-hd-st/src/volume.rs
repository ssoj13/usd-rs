//! HdStVolume - Storm volume prim implementation.
//!
//! Implements volume rendering for the Storm backend using raymarching.
//! Volumes can reference OpenVDB grids or Field3D assets.

use crate::draw_item::HdStDrawItemSharedPtr;
use std::sync::Arc;
use usd_sdf::Path as SdfPath;
use usd_tf::Token;

/// Default step size used for raymarching.
pub const DEFAULT_STEP_SIZE: f32 = 1.0;

/// Default step size used for raymarching for lighting computation.
pub const DEFAULT_STEP_SIZE_LIGHTING: f32 = 10.0;

/// Default memory limit for a field texture (in Mb) if not
/// overridden by field prim with textureMemory.
pub const DEFAULT_MAX_TEXTURE_MEMORY_PER_FIELD: f32 = 128.0;

/// Volume field binding.
#[derive(Debug, Clone)]
pub struct HdStVolumeFieldBinding {
    /// Field prim path
    pub field_prim_path: SdfPath,
    /// Field name (e.g., "density", "temperature")
    pub field_name: Token,
    /// Field index for arrays
    pub field_index: i32,
}

impl HdStVolumeFieldBinding {
    /// Create a new field binding.
    pub fn new(field_prim_path: SdfPath, field_name: Token) -> Self {
        Self {
            field_prim_path,
            field_name,
            field_index: 0,
        }
    }
}

/// Storm volume prim.
///
/// Represents a volume for raymarching-based rendering in Storm.
/// Volumes reference field prims (OpenVDB or Field3D) for actual data.
#[derive(Debug, Clone)]
pub struct HdStVolume {
    /// Prim path
    path: SdfPath,
    /// Field bindings (field name -> field prim path)
    field_bindings: Vec<HdStVolumeFieldBinding>,
    /// Step size for raymarching
    step_size: f32,
    /// Step size for lighting raymarching
    step_size_lighting: f32,
    /// Draw items for this prim
    draw_items: Vec<HdStDrawItemSharedPtr>,
    /// Whether volume is dirty
    dirty: bool,
}

impl HdStVolume {
    /// Create a new volume prim.
    pub fn new(path: SdfPath) -> Self {
        Self {
            path,
            field_bindings: Vec::new(),
            step_size: DEFAULT_STEP_SIZE,
            step_size_lighting: DEFAULT_STEP_SIZE_LIGHTING,
            draw_items: Vec::new(),
            dirty: true,
        }
    }

    /// Get the prim path.
    pub fn get_path(&self) -> &SdfPath {
        &self.path
    }

    /// Set field bindings.
    pub fn set_field_bindings(&mut self, bindings: Vec<HdStVolumeFieldBinding>) {
        self.field_bindings = bindings;
        self.dirty = true;
    }

    /// Get field bindings.
    pub fn get_field_bindings(&self) -> &[HdStVolumeFieldBinding] {
        &self.field_bindings
    }

    /// Add a field binding.
    pub fn add_field_binding(&mut self, binding: HdStVolumeFieldBinding) {
        self.field_bindings.push(binding);
        self.dirty = true;
    }

    /// Set step size for raymarching.
    pub fn set_step_size(&mut self, step_size: f32) {
        self.step_size = step_size;
    }

    /// Get step size.
    pub fn get_step_size(&self) -> f32 {
        self.step_size
    }

    /// Set step size for lighting raymarching.
    pub fn set_step_size_lighting(&mut self, step_size: f32) {
        self.step_size_lighting = step_size;
    }

    /// Get step size for lighting.
    pub fn get_step_size_lighting(&self) -> f32 {
        self.step_size_lighting
    }

    /// Sync the prim with scene delegate data.
    pub fn sync(&mut self) {
        if self.dirty {
            // Would load field textures and update shader bindings
            self.dirty = false;
        }
    }

    /// Finalize the prim.
    pub fn finalize(&mut self) {
        self.field_bindings.clear();
        self.draw_items.clear();
    }

    /// Get draw items for a representation.
    ///
    /// Returns draw items matching the given representation token.
    pub fn get_draw_items(&self, repr: &Token) -> Vec<HdStDrawItemSharedPtr> {
        self.draw_items
            .iter()
            .filter(|item| item.get_repr() == repr)
            .cloned()
            .collect()
    }

    /// Add a draw item.
    pub fn add_draw_item(&mut self, item: HdStDrawItemSharedPtr) {
        self.draw_items.push(item);
    }

    /// Get all draw items.
    pub fn get_all_draw_items(&self) -> &[HdStDrawItemSharedPtr] {
        &self.draw_items
    }
}

/// Shared pointer to Storm volume.
pub type HdStVolumeSharedPtr = Arc<HdStVolume>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_volume_creation() {
        let path = SdfPath::from_string("/volumes/smoke").unwrap();
        let volume = HdStVolume::new(path.clone());

        assert_eq!(volume.get_path(), &path);
        assert_eq!(volume.get_step_size(), DEFAULT_STEP_SIZE);
        assert_eq!(volume.get_step_size_lighting(), DEFAULT_STEP_SIZE_LIGHTING);
    }

    #[test]
    fn test_field_binding() {
        let path = SdfPath::from_string("/volumes/smoke").unwrap();
        let mut volume = HdStVolume::new(path);

        let field_path = SdfPath::from_string("/volumes/smoke/density").unwrap();
        let binding = HdStVolumeFieldBinding::new(field_path.clone(), Token::new("density"));

        volume.add_field_binding(binding);

        assert_eq!(volume.get_field_bindings().len(), 1);
        assert_eq!(volume.get_field_bindings()[0].field_prim_path, field_path);
    }

    #[test]
    fn test_step_size() {
        let path = SdfPath::from_string("/volumes/smoke").unwrap();
        let mut volume = HdStVolume::new(path);

        volume.set_step_size(0.5);
        volume.set_step_size_lighting(5.0);

        assert_eq!(volume.get_step_size(), 0.5);
        assert_eq!(volume.get_step_size_lighting(), 5.0);
    }

    #[test]
    fn test_volume_sync() {
        let path = SdfPath::from_string("/volumes/smoke").unwrap();
        let mut volume = HdStVolume::new(path);

        assert!(volume.dirty);
        volume.sync();
        assert!(!volume.dirty);
    }

    #[test]
    fn test_volume_finalize() {
        let path = SdfPath::from_string("/volumes/smoke").unwrap();
        let mut volume = HdStVolume::new(path);

        let field_path = SdfPath::from_string("/volumes/smoke/density").unwrap();
        volume.add_field_binding(HdStVolumeFieldBinding::new(
            field_path,
            Token::new("density"),
        ));

        volume.finalize();

        assert!(volume.get_field_bindings().is_empty());
        assert!(volume.get_all_draw_items().is_empty());
    }
}
