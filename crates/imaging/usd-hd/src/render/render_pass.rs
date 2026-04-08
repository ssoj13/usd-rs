//! HdRenderPass - Execute rendering for a set of prims.
//!
//! Represents a single render iteration over a collection of prims with
//! specific camera/viewport parameters.
//!
//! # Execution Model
//!
//! Render passes have two phases:
//! 1. **Sync** - Update internal caching based on collection changes
//! 2. **Execute** - Perform actual rendering with given render pass state
//!
//! # Collection-Based Filtering
//!
//! Each render pass operates on an HdRprimCollection which defines:
//! - Which prims to include
//! - Representation mode (hull, refined, etc.)
//! - Material override options

use super::render_delegate::HdRprimCollection;
use super::render_index::HdRenderIndex;
use std::sync::Arc;

// Forward declarations
pub use super::task::TfTokenVector;

/// Shared pointer to render pass state.
pub type HdRenderPassStateSharedPtr = Arc<dyn HdRenderPassState>;

/// Render pass state containing camera and viewport parameters.
///
/// Passed to Execute() to configure rendering.
/// Full implementation will be in render_pass_state.rs (future).
pub trait HdRenderPassState: Send + Sync {
    /// Get camera path.
    fn get_camera(&self) -> Option<&usd_sdf::Path>;

    /// For downcasting to concrete types (e.g. HdStRenderPassState).
    /// Default returns a reference that cannot be downcast; implementors
    /// should override with `self` for downcast support.
    fn as_any(&self) -> &dyn std::any::Any {
        static PLACEHOLDER: () = ();
        &PLACEHOLDER
    }

    /// Get viewport dimensions.
    fn get_viewport(&self) -> (f32, f32, f32, f32) {
        (0.0, 0.0, 1920.0, 1080.0)
    }

    /// Prepare (update parameters before render).
    ///
    /// Matches C++ `HdRenderPassState::Prepare(HdResourceRegistrySharedPtr)`.
    /// Called once per frame after sync phase, prior to commit.
    /// Default is no-op; backends override to upload GPU resources.
    fn prepare(
        &mut self,
        _resource_registry: &std::sync::Arc<dyn super::render_delegate::HdResourceRegistry>,
    ) {
        // Base implementation: no-op
    }
}

/// Abstract render pass for executing rendering over a prim collection.
///
/// Backends specialize this to implement their rendering strategy.
///
/// # Example
/// ```ignore
/// use usd_hd::render::*;
///
/// struct MyRenderPass {
///     collection: HdRprimCollection,
///     collection_dirty: bool,
/// }
///
/// impl HdRenderPass for MyRenderPass {
///     fn get_rprim_collection(&self) -> &HdRprimCollection {
///         &self.collection
///     }
///     
///     fn sync(&mut self) {
///         if self.collection_dirty {
///             // Rebuild draw list
///             self.collection_dirty = false;
///         }
///     }
///     
///     fn execute(&mut self, state: &HdRenderPassStateSharedPtr, tags: &[Token]) {
///         // Iterate over draw items and issue GPU commands
///     }
/// }
/// ```
pub trait HdRenderPass: Send + Sync {
    /// Get the rprim collection this pass renders.
    fn get_rprim_collection(&self) -> &HdRprimCollection;

    /// Set the rprim collection (may invalidate internal caches).
    fn set_rprim_collection(&mut self, collection: HdRprimCollection);

    /// Get the render index (matches C++ GetRenderIndex()).
    /// Returns a raw pointer stored at construction time.
    fn get_render_index(&self) -> Option<*mut HdRenderIndex> {
        None
    }

    /// Sync the render pass resources.
    ///
    /// Called to update internal state based on collection changes.
    /// Base implementation enqueues the collection to the render index
    /// for rprim sync (matches C++ HdRenderPass::Sync).
    fn sync(&mut self);

    /// Execute rendering with the given state and render tags.
    ///
    /// # Parameters
    /// - `state` - Render pass state (camera, viewport, etc.)
    /// - `render_tags` - Filter prims by render tags (empty = render all)
    fn execute(&mut self, state: &HdRenderPassStateSharedPtr, render_tags: &TfTokenVector);

    /// Check if render pass is converged (for progressive rendering).
    fn is_converged(&self) -> bool {
        true
    }
}

/// Base implementation providing common render pass functionality.
///
/// Stores a raw pointer to the render index (matches C++ `HdRenderIndex * const`).
/// The render index must outlive the render pass.
pub struct HdRenderPassBase {
    /// The collection of prims to render
    collection: HdRprimCollection,

    /// Flag indicating collection has changed
    collection_dirty: bool,

    /// Render index pointer (matches C++ _renderIndex).
    /// Set at construction, must outlive this render pass.
    render_index: *mut HdRenderIndex,
}

// SAFETY: The render index pointer follows the same lifetime contract as C++:
// the index outlives the render pass, and access is single-threaded during sync/execute.
// The pointer is never dereferenced concurrently.
#[allow(unsafe_code)]
unsafe impl Send for HdRenderPassBase {}
#[allow(unsafe_code)]
unsafe impl Sync for HdRenderPassBase {}

impl HdRenderPassBase {
    /// Create a new render pass base with render index (matches C++ constructor).
    pub fn new_with_index(index: &mut HdRenderIndex, collection: HdRprimCollection) -> Self {
        Self {
            collection,
            collection_dirty: true,
            render_index: index as *mut HdRenderIndex,
        }
    }

    /// Create a new render pass base without render index (for tests/backwards compat).
    pub fn new(collection: HdRprimCollection) -> Self {
        Self {
            collection,
            collection_dirty: true,
            render_index: std::ptr::null_mut(),
        }
    }

    /// Get the render index pointer.
    pub fn get_render_index(&self) -> Option<*mut HdRenderIndex> {
        if self.render_index.is_null() {
            None
        } else {
            Some(self.render_index)
        }
    }

    /// Get the rprim collection.
    pub fn get_rprim_collection(&self) -> &HdRprimCollection {
        &self.collection
    }

    /// Set the rprim collection.
    ///
    /// Matches C++ `HdRenderPass::SetRprimCollection` (renderPass.cpp:33-43).
    /// Uses full equality check (not just name) - skips update if unchanged.
    pub fn set_rprim_collection(&mut self, collection: HdRprimCollection) {
        if collection == self.collection {
            return;
        }
        self.collection = collection;
        self.collection_dirty = true;
    }

    /// Base sync: enqueue collection to render index for rprim sync.
    ///
    /// Matches C++ HdRenderPass::Sync() which calls
    /// `_renderIndex->EnqueueCollectionToSync(_collection)`.
    pub fn sync(&mut self) {
        if !self.render_index.is_null() {
            // SAFETY: render index pointer is valid for the lifetime of this pass
            #[allow(unsafe_code)]
            let index = unsafe { &mut *self.render_index };
            index.enqueue_collection_to_sync(self.collection.clone());
        }
    }

    /// Check if collection is dirty.
    pub fn is_collection_dirty(&self) -> bool {
        self.collection_dirty
    }

    /// Mark collection as dirty.
    pub fn mark_collection_dirty(&mut self) {
        self.collection_dirty = true;
    }

    /// Mark collection as clean.
    pub fn mark_collection_clean(&mut self) {
        self.collection_dirty = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use usd_tf::Token;

    struct TestRenderPass {
        base: HdRenderPassBase,
        sync_count: usize,
        execute_count: usize,
    }

    impl TestRenderPass {
        fn new(collection: HdRprimCollection) -> Self {
            Self {
                base: HdRenderPassBase::new(collection),
                sync_count: 0,
                execute_count: 0,
            }
        }
    }

    impl HdRenderPass for TestRenderPass {
        fn get_rprim_collection(&self) -> &HdRprimCollection {
            self.base.get_rprim_collection()
        }

        fn set_rprim_collection(&mut self, collection: HdRprimCollection) {
            self.base.set_rprim_collection(collection);
        }

        fn sync(&mut self) {
            self.sync_count += 1;
            if self.base.is_collection_dirty() {
                // Rebuild draw list
                self.base.mark_collection_clean();
            }
        }

        fn execute(&mut self, _state: &HdRenderPassStateSharedPtr, _tags: &TfTokenVector) {
            self.execute_count += 1;
        }
    }

    #[test]
    fn test_render_pass_creation() {
        let collection = HdRprimCollection::new(Token::new("geometry"));
        let pass = TestRenderPass::new(collection);

        assert_eq!(pass.get_rprim_collection().name.as_str(), "geometry");
        assert!(pass.base.is_collection_dirty());
    }

    #[test]
    fn test_render_pass_sync() {
        let collection = HdRprimCollection::new(Token::new("test"));
        let mut pass = TestRenderPass::new(collection);

        pass.sync();
        assert_eq!(pass.sync_count, 1);
        assert!(!pass.base.is_collection_dirty());
    }

    #[test]
    fn test_collection_change() {
        let collection1 = HdRprimCollection::new(Token::new("col1"));
        let mut pass = TestRenderPass::new(collection1);

        pass.sync();
        assert!(!pass.base.is_collection_dirty());

        let collection2 = HdRprimCollection::new(Token::new("col2"));
        pass.set_rprim_collection(collection2);
        assert!(pass.base.is_collection_dirty());
    }
}
