
//! HdStRenderParam - Storm render parameter.
//!
//! This contains Storm-specific state passed to prims during sync.

use usd_hd::render::HdRenderParam;
use usd_tf::Token;
use usd_vt::Value;

/// Storm render parameter.
///
/// Contains Storm-specific rendering state that is passed to prims
/// during synchronization. This allows prims to access shared rendering
/// resources and state without requiring global variables.
#[derive(Debug, Default)]
pub struct HdStRenderParam {
    /// Draw items cache dirty flag
    draw_items_cache_dirty: bool,

    /// Garbage collection needed flag
    needs_garbage_collection: bool,

    /// Draw batches dirty flag
    draw_batches_dirty: bool,

    /// Material tags dirty flag
    material_tags_dirty: bool,

    /// Geom subset draw items dirty flag
    geom_subset_draw_items_dirty: bool,

    /// Monotonic version counter for material tags (incremented on dirty)
    material_tags_version: usize,

    /// Monotonic version counter for geom subset draw items
    geom_subset_draw_items_version: usize,

    /// Known material tags for filtering
    material_tags: std::collections::HashSet<Token>,

    /// Known render tags for filtering
    render_tags: std::collections::HashSet<Token>,
}

impl HdStRenderParam {
    /// Create a new Storm render param.
    pub fn new() -> Self {
        Self {
            draw_items_cache_dirty: false,
            needs_garbage_collection: false,
            draw_batches_dirty: false,
            material_tags_dirty: false,
            geom_subset_draw_items_dirty: false,
            material_tags_version: 0,
            geom_subset_draw_items_version: 0,
            material_tags: std::collections::HashSet::new(),
            render_tags: std::collections::HashSet::new(),
        }
    }

    /// Mark draw items cache as dirty.
    pub fn mark_draw_items_cache_dirty(&mut self) {
        self.draw_items_cache_dirty = true;
    }

    /// Check if draw items cache is dirty.
    pub fn is_draw_items_cache_dirty(&self) -> bool {
        self.draw_items_cache_dirty
    }

    /// Clear draw items cache dirty flag.
    pub fn clear_draw_items_cache_dirty(&mut self) {
        self.draw_items_cache_dirty = false;
    }

    /// Mark that garbage collection is needed.
    pub fn mark_garbage_collection_needed(&mut self) {
        self.needs_garbage_collection = true;
    }

    /// Check if garbage collection is needed.
    pub fn needs_gc(&self) -> bool {
        self.needs_garbage_collection
    }

    /// Clear garbage collection flag.
    pub fn clear_gc_flag(&mut self) {
        self.needs_garbage_collection = false;
    }

    /// Mark draw batches as dirty (triggers rebuild).
    pub fn mark_draw_batches_dirty(&mut self) {
        self.draw_batches_dirty = true;
    }

    /// Check if draw batches are dirty.
    pub fn is_draw_batches_dirty(&self) -> bool {
        self.draw_batches_dirty
    }

    /// Clear draw batches dirty flag.
    pub fn clear_draw_batches_dirty(&mut self) {
        self.draw_batches_dirty = false;
    }

    /// Mark material tags as dirty (triggers re-bucketing).
    pub fn mark_material_tags_dirty(&mut self) {
        self.material_tags_dirty = true;
        self.material_tags_version += 1;
    }

    /// Check if material tags are dirty.
    pub fn is_material_tags_dirty(&self) -> bool {
        self.material_tags_dirty
    }

    /// Clear material tags dirty flag.
    pub fn clear_material_tags_dirty(&mut self) {
        self.material_tags_dirty = false;
    }

    /// Mark geometry subset draw items as dirty.
    pub fn mark_geom_subset_draw_items_dirty(&mut self) {
        self.geom_subset_draw_items_dirty = true;
        self.geom_subset_draw_items_version += 1;
    }

    /// Get material tags version counter.
    pub fn get_material_tags_version(&self) -> usize {
        self.material_tags_version
    }

    /// Get geom subset draw items version counter.
    pub fn get_geom_subset_draw_items_version(&self) -> usize {
        self.geom_subset_draw_items_version
    }

    /// Register a material tag.
    pub fn add_material_tag(&mut self, tag: &Token) {
        self.material_tags.insert(tag.clone());
    }

    /// Check if a material tag is known.
    pub fn has_material_tag(&self, tag: &Token) -> bool {
        self.material_tags.contains(tag)
    }

    /// Register a render tag.
    pub fn add_render_tag(&mut self, tag: &Token) {
        self.render_tags.insert(tag.clone());
    }

    /// Check if any of the given render tags are known.
    pub fn has_any_render_tag(&self, tags: &[Token]) -> bool {
        tags.iter().any(|t| self.render_tags.contains(t))
    }

    /// Check if geom subset draw items are dirty.
    pub fn is_geom_subset_draw_items_dirty(&self) -> bool {
        self.geom_subset_draw_items_dirty
    }

    /// Clear geom subset draw items dirty flag.
    pub fn clear_geom_subset_draw_items_dirty(&mut self) {
        self.geom_subset_draw_items_dirty = false;
    }
}

impl HdRenderParam for HdStRenderParam {
    fn set_arbitrary_value(&mut self, _key: &Token, _value: &Value) -> bool {
        // Storm render param currently doesn't support custom values
        // Could be extended in the future
        false
    }

    fn get_arbitrary_value(&self, _key: &Token) -> Option<Value> {
        None
    }

    fn has_arbitrary_value(&self, _key: &Token) -> bool {
        false
    }

    fn is_valid(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_param_creation() {
        let param = HdStRenderParam::new();
        assert!(param.is_valid());
        assert!(!param.is_draw_items_cache_dirty());
        assert!(!param.needs_gc());
    }

    #[test]
    fn test_draw_items_cache_dirty() {
        let mut param = HdStRenderParam::new();
        assert!(!param.is_draw_items_cache_dirty());

        param.mark_draw_items_cache_dirty();
        assert!(param.is_draw_items_cache_dirty());

        param.clear_draw_items_cache_dirty();
        assert!(!param.is_draw_items_cache_dirty());
    }

    #[test]
    fn test_garbage_collection() {
        let mut param = HdStRenderParam::new();
        assert!(!param.needs_gc());

        param.mark_garbage_collection_needed();
        assert!(param.needs_gc());

        param.clear_gc_flag();
        assert!(!param.needs_gc());
    }
}
