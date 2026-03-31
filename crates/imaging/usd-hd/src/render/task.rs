
//! HdTask - Unit of work in Hydra's rendering pipeline.
//!
//! Tasks represent discrete rendering operations that can be composed into
//! a render graph. Examples include draw tasks, resolve tasks, and post-processing.
//!
//! # Task Execution Model
//!
//! Tasks execute in three phases:
//!
//! 1. **Sync** - Pull changed data from scene delegate (may run in parallel)
//! 2. **Prepare** - Resolve bindings, create resources (runs in execution order)
//! 3. **Execute** - Perform rendering work (runs in execution order, non-parallel)
//!
//! # Inter-Task Communication
//!
//! Tasks communicate via HdTaskContext:
//! - Prepare phase can store data for Execute phase
//! - Tasks can share resources (camera matrices, render targets, etc.)
//! - Context doesn't persist across HdEngine::Execute() calls

use parking_lot::RwLock;
use std::any::Any;
use std::sync::Arc;

use usd_sdf::Path as SdfPath;
use usd_tf::Token;

use super::render_index::HdPrimHandle;
use super::task_context::HdTaskContext;
use crate::change_tracker::HdChangeTracker;
use crate::types::HdDirtyBits;

// Forward declarations
pub use crate::prim::HdSceneDelegate;

/// Shared pointer to a task.
/// Arc<RwLock<..>> enables &mut self access through shared ownership
/// (needed for prepare/execute phases in HdEngine).
pub type HdTaskSharedPtr = Arc<RwLock<dyn HdTask>>;

/// Vector of task shared pointers.
pub type HdTaskSharedPtrVector = Vec<HdTaskSharedPtr>;

/// Render tags vector for filtering which prims to render.
pub type TfTokenVector = Vec<Token>;

/// Trait exposing HdRenderIndex API to tasks.
///
/// In C++ `HdTask::Prepare` takes `HdRenderIndex*` which has full access
/// to prim queries, change tracker, render delegate, etc.
/// This trait mirrors that surface so tasks can query the scene.
pub trait HdRenderIndex {
    /// Get a task by path.
    fn get_task(&self, id: &SdfPath) -> Option<&HdTaskSharedPtr>;

    /// Check if a task exists.
    fn has_task(&self, id: &SdfPath) -> bool;

    /// Get an rprim handle by path.
    fn get_rprim(&self, id: &SdfPath) -> Option<&HdPrimHandle>;

    /// Get all rprim paths currently in the render index.
    fn get_rprim_ids(&self) -> Vec<SdfPath>;

    /// Get the integer prim ID assigned to an rprim path.
    fn get_prim_id_for_rprim_path(&self, rprim_path: &SdfPath) -> Option<i32>;

    /// Get an sprim handle by type and path.
    fn get_sprim(&self, type_id: &Token, id: &SdfPath) -> Option<&HdPrimHandle>;

    /// Get a bprim handle by type and path.
    fn get_bprim(&self, type_id: &Token, id: &SdfPath) -> Option<&HdPrimHandle>;

    /// Get the render delegate (as Arc<RwLock<dyn HdRenderDelegate>>).
    fn get_render_delegate(&self) -> &super::render_index::HdRenderDelegateSharedPtr;

    /// Get the change tracker.
    fn get_change_tracker(&self) -> &HdChangeTracker;
}

/// Unit of work in Hydra's rendering pipeline.
///
/// Tasks can prepare resources, execute rendering, or coordinate with other systems.
///
/// # Example
/// ```ignore
/// use usd_hd::render::{HdTask, HdTaskContext};
/// use usd_sdf::Path;
///
/// struct MyDrawTask {
///     id: Path,
///     dirty_bits: u32,
/// }
///
/// impl HdTask for MyDrawTask {
///     fn id(&self) -> &Path {
///         &self.id
///     }
///     
///     fn sync(&mut self, delegate: &dyn HdSceneDelegate,
///             ctx: &mut HdTaskContext, dirty_bits: &mut u32) {
///         // Pull changed data from scene delegate
///         *dirty_bits = 0; // Mark clean
///     }
///     
///     fn prepare(&mut self, ctx: &mut HdTaskContext,
///                render_index: &dyn HdRenderIndex) {
///         // Resolve bindings, create resources
///     }
///     
///     fn execute(&mut self, ctx: &mut HdTaskContext) {
///         // Perform rendering
///     }
/// }
/// ```
pub trait HdTask: Send + Sync {
    /// Get the task's scene path identifier.
    fn id(&self) -> &SdfPath;

    /// Check if the task considers its execution converged.
    ///
    /// For progressive rendering, returns true when the task has finished.
    /// Returns true by default for non-progressive tasks.
    fn is_converged(&self) -> bool {
        true
    }

    /// Sync phase: Pull changed data from scene delegate.
    ///
    /// Called when dirty_bits is non-zero. Tasks may be synced in parallel and out of order.
    ///
    /// # Parameters
    /// - `delegate` - Scene delegate for querying prim data
    /// - `ctx` - Task context (legacy, prefer not to use in Sync)
    /// - `dirty_bits` - Mutable dirty bits to clear after processing
    ///
    /// # Note
    /// Don't access other prims during Sync - they may not be synced yet.
    /// Store paths and resolve them in Prepare phase instead.
    fn sync(
        &mut self,
        delegate: &dyn HdSceneDelegate,
        ctx: &mut HdTaskContext,
        dirty_bits: &mut HdDirtyBits,
    );

    /// Prepare phase: Resolve bindings and manage resources.
    ///
    /// Called after all tasks are synced, in execution order.
    /// Use this to:
    /// - Query other prims via render_index
    /// - Create/update temporary resources
    /// - Store data in task context for Execute phase
    ///
    /// # Parameters
    /// - `ctx` - Task context for inter-task communication
    /// - `render_index` - Render index for querying scene state
    fn prepare(&mut self, ctx: &mut HdTaskContext, render_index: &dyn HdRenderIndex);

    /// Execute phase: Perform rendering work.
    ///
    /// Called in execution order, non-parallel. Should trigger actual render
    /// delegate processing (draw commands, compute dispatches, etc).
    ///
    /// # Parameters
    /// - `ctx` - Task context (same as Prepare phase)
    fn execute(&mut self, ctx: &mut HdTaskContext);

    /// Gather render tags for this task.
    ///
    /// Called during Sync phase after the task has been synced.
    /// Returns render tags to be added to the active set.
    ///
    /// C++ returns `const TfTokenVector&`. We return `&[Token]` to avoid
    /// allocating a Vec on every call. Implementors store tags internally.
    fn get_render_tags(&self) -> &[Token] {
        &[]
    }

    /// Returns the minimal set of dirty bits for the first sync.
    ///
    /// Matches C++ `HdTask::GetInitialDirtyBitsMask` (task.cpp:44-48).
    /// Default: DirtyParams | DirtyCollection | DirtyRenderTags.
    fn get_initial_dirty_bits_mask(&self) -> HdDirtyBits {
        HdTaskBase::get_initial_dirty_bits_mask()
    }

    /// Downcast to concrete type via Any.
    /// Required for HdxTaskController to update task-specific params.
    fn as_any(&self) -> &dyn Any;

    /// Downcast to concrete type via Any (mutable).
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

/// Base implementation for common task functionality.
///
/// Provides default implementations and utilities for concrete task types.
/// Also provides protected helper methods matching C++ HdTask protected API.
pub struct HdTaskBase {
    /// Task identifier
    id: SdfPath,
}

impl HdTaskBase {
    /// Create a new task base with the given id.
    pub fn new(id: SdfPath) -> Self {
        Self { id }
    }

    /// Get the task id.
    pub fn id(&self) -> &SdfPath {
        &self.id
    }

    /// Returns the minimal set of dirty bits for first sync.
    ///
    /// Matches C++ `HdTask::GetInitialDirtyBitsMask` (task.cpp:44-48).
    /// Returns DirtyParams | DirtyCollection | DirtyRenderTags.
    pub fn get_initial_dirty_bits_mask() -> HdDirtyBits {
        use crate::change_tracker::HdTaskDirtyBits;
        HdTaskDirtyBits::DIRTY_PARAMS
            | HdTaskDirtyBits::DIRTY_COLLECTION
            | HdTaskDirtyBits::DIRTY_RENDER_TAGS
    }

    /// Check if the shared context contains a value for the given id.
    ///
    /// Matches C++ `HdTask::_HasTaskContextData` (task.cpp:52-58).
    pub fn has_task_context_data(ctx: &HdTaskContext, id: &Token) -> bool {
        ctx.contains_key(id)
    }

    /// Extract a typed value from task context.
    ///
    /// Matches C++ `HdTask::_GetTaskContextData<T>` (task.h:178-181).
    /// Returns None if key is missing or type doesn't match.
    pub fn get_task_context_data<'a>(
        ctx: &'a HdTaskContext,
        id: &Token,
    ) -> Option<&'a usd_vt::Value> {
        ctx.get(id)
    }

    /// Extract task params from the scene delegate.
    ///
    /// Matches C++ `HdTask::_GetTaskParams<T>` (task.h:191-192).
    /// Gets the "params" value for this task from the delegate.
    pub fn get_task_params(delegate: &dyn HdSceneDelegate, task_id: &SdfPath) -> usd_vt::Value {
        delegate.get(task_id, &Token::new("params"))
    }

    /// Get render tags for this task from the scene delegate.
    ///
    /// Matches C++ `HdTask::_GetTaskRenderTags` (task.cpp:60-63).
    pub fn get_task_render_tags(
        delegate: &dyn HdSceneDelegate,
        task_id: &SdfPath,
    ) -> TfTokenVector {
        delegate.get_task_render_tags(task_id)
    }

    /// Extract a driver object from task context.
    ///
    /// Matches C++ `HdTask::_GetDriver<T>` (task.h:199-202).
    /// Looks up the drivers in the task context and finds one by name.
    pub fn get_driver<'a>(
        ctx: &'a HdTaskContext,
        driver_name: &Token,
    ) -> Option<&'a usd_vt::Value> {
        // Drivers are stored separately in HdTaskContext
        if let Some(drivers) = ctx.get_drivers() {
            for driver in drivers {
                if driver.name() == driver_name {
                    return Some(driver.driver());
                }
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestTask {
        base: HdTaskBase,
        sync_count: usize,
        prepare_count: usize,
        execute_count: usize,
    }

    impl TestTask {
        fn new(id: SdfPath) -> Self {
            Self {
                base: HdTaskBase::new(id),
                sync_count: 0,
                prepare_count: 0,
                execute_count: 0,
            }
        }
    }

    impl HdTask for TestTask {
        fn id(&self) -> &SdfPath {
            self.base.id()
        }

        fn sync(
            &mut self,
            _delegate: &dyn HdSceneDelegate,
            _ctx: &mut HdTaskContext,
            dirty_bits: &mut HdDirtyBits,
        ) {
            self.sync_count += 1;
            *dirty_bits = 0;
        }

        fn prepare(&mut self, _ctx: &mut HdTaskContext, _render_index: &dyn HdRenderIndex) {
            self.prepare_count += 1;
        }

        fn execute(&mut self, _ctx: &mut HdTaskContext) {
            self.execute_count += 1;
        }

        fn as_any(&self) -> &dyn Any {
            self
        }

        fn as_any_mut(&mut self) -> &mut dyn Any {
            self
        }
    }

    #[test]
    fn test_task_creation() {
        let id = SdfPath::from_string("/Render/DrawTask").unwrap();
        let task = TestTask::new(id.clone());

        assert_eq!(task.id().as_str(), "/Render/DrawTask");
        assert!(task.is_converged());
        assert_eq!(task.sync_count, 0);
    }

    #[test]
    fn test_task_base() {
        let id = SdfPath::from_string("/Test").unwrap();
        let base = HdTaskBase::new(id.clone());

        assert_eq!(base.id().as_str(), "/Test");
    }
}
