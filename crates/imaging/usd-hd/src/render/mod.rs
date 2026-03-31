
//! Hydra rendering infrastructure.
//!
//! This module provides the core rendering execution system for Hydra:
//!
//! # Core Components
//!
//! - **HdEngine** - Top-level entry point for executing rendering tasks
//! - **HdRenderIndex** - Central registry for all scene primitives
//! - **HdRenderDelegate** - Main extension point for rendering backends
//! - **HdRenderPass** - Execute rendering for a set of prims
//! - **HdTask** - Unit of work in the rendering pipeline
//! - **HdTaskContext** - Shared state for inter-task communication
//! - **HdDriver** - GPU device handle wrapper
//!
//! # Architecture
//!
//! The rendering system follows a trait-based design:
//!
//! ```text
//! HdEngine
//!   └─> Executes HdTask vector
//!       ├─> Tasks use HdTaskContext for communication
//!       └─> Tasks query HdRenderIndex for scene data
//!           └─> RenderIndex uses HdRenderDelegate to create prims
//!               └─> Delegate creates backend-specific objects
//! ```
//!
//! # Task Execution Model
//!
//! Tasks execute in three phases:
//!
//! 1. **Sync** - Pull changed data from scene delegates (parallel)
//! 2. **Prepare** - Resolve bindings, create resources (ordered)
//! 3. **Execute** - Perform rendering work (ordered, non-parallel)
//!
//! # Render Delegate Pattern
//!
//! Backends implement `HdRenderDelegate` to provide:
//! - Prim creation factories
//! - Supported prim type lists
//! - Render pass creation
//! - Resource registry access
//! - Render settings management
//!
//! # Example Usage
//!
//! ```ignore
//! use usd_hd::render::*;
//! use std::sync::Arc;
//! use parking_lot::RwLock;
//!
//! // Create render delegate (backend-specific)
//! let delegate = Arc::new(RwLock::new(MyRenderDelegate::new()));
//!
//! // Create render index
//! let mut index = HdRenderIndex::new(
//!     delegate,
//!     vec![hgi_driver],
//!     Some("mainIndex".to_string()),
//!     None,
//! ).unwrap();
//!
//! // Create engine
//! let mut engine = HdEngine::new();
//!
//! // Set external data in task context
//! engine.set_task_context_data(
//!     Token::new("aovBindings"),
//!     Value::from(aov_bindings)
//! );
//!
//! // Execute render tasks
//! let mut tasks = vec![draw_task, resolve_task];
//! engine.execute(&mut index, &mut tasks);
//! ```
//!
//! # Render Pass Usage
//!
//! ```ignore
//! // Create a collection of prims to render
//! let collection = HdRprimCollection::new(Token::new("geometry"));
//!
//! // Create render pass from delegate
//! let render_pass = delegate.create_render_pass(&index, &collection);
//!
//! // Sync and execute
//! render_pass.sync();
//! render_pass.execute(&render_state, &render_tags);
//! ```

pub mod driver;
pub mod engine;
pub mod render_delegate;
pub mod render_index;
pub mod render_pass;
pub mod rprim_collection;
pub mod task;
pub mod task_context;

// Re-export main types
pub use driver::{HdDriver, HdDriverVector};
pub use render_delegate::{
    HdInstancer, HdRenderDelegate, HdRenderDelegateBase, HdRenderParam, HdRenderParamSharedPtr,
    HdRenderPass as HdRenderPassTrait, HdRenderSettingDescriptor, HdRenderSettingDescriptorList,
    HdRenderSettingsMap, HdResourceRegistry, HdResourceRegistrySharedPtr,
};
pub use rprim_collection::HdRprimCollection;
pub use task::{
    HdRenderIndex as HdRenderIndexTrait, HdTask, HdTaskBase, HdTaskSharedPtr,
    HdTaskSharedPtrVector, TfTokenVector,
};
pub use task_context::HdTaskContext;

/// Shared pointer to render pass using the full HdRenderPass trait.
pub type HdRenderPassSharedPtr = std::sync::Arc<dyn HdRenderPass>;
pub use engine::HdEngine;
pub use render_index::{
    // Adapter wrappers: Box<BprimAdapter<T>> implements HdBprimSync for any HdBprim T
    BprimAdapter,
    // Object-safe sync handles — for direct prim.sync() dispatch
    HdBprimHandle,
    HdBprimSync,
    // Opaque handle (Box<dyn Any>) — for render_delegate create/destroy/get API
    HdPrimHandle,
    HdRenderDelegateSharedPtr,
    HdRenderIndex,
    HdRprimHandle,
    HdRprimSync,
    HdSprimHandle,
    HdSprimSync,
    RprimAdapter,
    SprimAdapter,
};
pub use render_pass::{
    HdRenderPass, HdRenderPassBase, HdRenderPassState, HdRenderPassStateSharedPtr,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_module_exports() {
        // Verify main types are accessible
        let _engine = HdEngine::new();
        let _context = HdTaskContext::new();
        let _collection = HdRprimCollection::new(usd_tf::Token::new("test"));
    }
}
