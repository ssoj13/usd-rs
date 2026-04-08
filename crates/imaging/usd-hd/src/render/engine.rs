//! HdEngine - Top-level entry point for executing Hydra rendering.
//!
//! The engine executes render tasks and manages the task context.
//! Typically, applications create one engine instance.
//!
//! # Task Execution Phases (matches C++ HdEngine::Execute)
//!
//! 1. **Data Discovery** - SyncAll on render index (pulls changed data)
//! 2. **Prepare** - task->Prepare() in execution order (resolve bindings)
//! 3. **Data Commit** - renderDelegate->CommitResources() (upload to GPU)
//! 4. **Execute** - task->Execute() in execution order (render)
//!
//! # Task Context
//!
//! The engine maintains a persistent task context (`HdTaskContext`) for
//! inter-task communication. Applications can pre-populate it with external
//! data (e.g., AOV bindings, camera overrides). The context persists across
//! Execute() calls.

use super::render_index::HdRenderIndex;
use super::task::HdTaskSharedPtrVector;
use super::task_context::HdTaskContext;
use usd_sdf::Path as SdfPath;
use usd_tf::Token;
use usd_vt::Value;

/// Top-level rendering execution engine.
///
/// Matches C++ `HdEngine`. Owns a persistent `HdTaskContext` and orchestrates
/// the sync/prepare/commit/execute pipeline.
pub struct HdEngine {
    /// Persistent task context for inter-task communication.
    /// Populated by the engine (drivers) and externally by the application.
    task_context: HdTaskContext,
}

impl HdEngine {
    /// Create a new engine with empty task context.
    pub fn new() -> Self {
        Self {
            task_context: HdTaskContext::new(),
        }
    }

    //--------------------------------------------------------------------------
    // Task Context Management (C++ parity: SetTaskContextData, etc.)
    //--------------------------------------------------------------------------

    /// Add or update a value in the task context.
    ///
    /// Matches C++ `HdEngine::SetTaskContextData`.
    pub fn set_task_context_data(&mut self, id: Token, data: Value) {
        self.task_context.insert(id, data);
    }

    /// Get a value from the task context. Returns None if not found.
    ///
    /// Matches C++ `HdEngine::GetTaskContextData` (returns bool + out-param).
    pub fn get_task_context_data(&self, id: &Token) -> Option<&Value> {
        self.task_context.get(id)
    }

    /// Remove a value from the task context.
    ///
    /// Matches C++ `HdEngine::RemoveTaskContextData`.
    pub fn remove_task_context_data(&mut self, id: &Token) {
        self.task_context.remove(id);
    }

    /// Clear all task context data.
    ///
    /// Matches C++ `HdEngine::ClearTaskContextData`.
    pub fn clear_task_context_data(&mut self) {
        self.task_context.clear();
    }

    //--------------------------------------------------------------------------
    // Task Execution (C++ parity: Execute, AreTasksConverged)
    //--------------------------------------------------------------------------

    /// Execute tasks through all phases: Sync, Prepare, Commit, Execute.
    ///
    /// Matches C++ `HdEngine::Execute(HdRenderIndex*, HdTaskSharedPtrVector*)`.
    ///
    /// # Phases
    ///
    /// 1. **Data Discovery** - Calls `index.sync_all(tasks, &task_context)`.
    ///    Discovers required input data, populates resource registry with
    ///    buffer sources and computations.
    ///
    /// 2. **Prepare** - Calls `task.prepare(&task_context, index)` for each
    ///    task in execution order. Tasks resolve bindings and manage resources.
    ///
    /// 3. **Data Commit** - Calls `render_delegate.commit_resources()` via
    ///    the render index. Resources are uploaded to CPU/GPU.
    ///
    /// 4. **Execute** - Calls `task.execute(&task_context)` for each task
    ///    in execution order. Triggers actual rendering.
    pub fn execute(&mut self, index: &mut HdRenderIndex, tasks: &mut HdTaskSharedPtrVector) {
        if tasks.is_empty() {
            return;
        }

        log::debug!("[hd_engine] execute: {} tasks", tasks.len());
        // Inject drivers into task context
        self.task_context.set_drivers(index.get_drivers().clone());

        // DATA DISCOVERY PHASE
        log::debug!("[hd_engine] sync_all start");
        index.sync_all(tasks.as_mut_slice(), &mut self.task_context);
        log::debug!("[hd_engine] sync_all done");

        // PREPARE PHASE
        for (i, task) in tasks.iter().enumerate() {
            let mut guard = task.write();
            log::trace!("[hd_engine] prepare task {}: {}", i, guard.id());
            guard.prepare(&mut self.task_context, index);
        }
        log::debug!("[hd_engine] prepare done");

        // DATA COMMIT PHASE
        index.commit_resources();
        log::debug!("[hd_engine] commit done");

        // EXECUTE PHASE
        for (i, task) in tasks.iter().enumerate() {
            let mut guard = task.write();
            log::trace!("[hd_engine] execute task {}: {}", i, guard.id());
            guard.execute(&mut self.task_context);
        }
        log::debug!("[hd_engine] execute done");
    }

    /// Execute tasks specified by their scene paths.
    ///
    /// Matches C++ `HdEngine::Execute(HdRenderIndex*, const SdfPathVector&)`.
    /// Looks up tasks from the render index, builds a task vector, then
    /// delegates to the main `execute()`.
    pub fn execute_by_paths(&mut self, index: &mut HdRenderIndex, task_paths: &[SdfPath]) {
        // Build task vector from paths (matches C++ implementation)
        let mut tasks: HdTaskSharedPtrVector = Vec::with_capacity(task_paths.len());
        for path in task_paths {
            if path.is_empty() {
                eprintln!("Warning: Empty task path given to HdEngine::execute_by_paths()");
                continue;
            }
            if let Some(task) = index.get_task(path) {
                tasks.push(task.clone());
            } else {
                eprintln!(
                    "Warning: No task at '{}' in render index in HdEngine::execute_by_paths()",
                    path.as_str()
                );
            }
        }
        self.execute(index, &mut tasks);
    }

    /// Check if all tasks at the given paths are converged.
    ///
    /// Matches C++ `HdEngine::AreTasksConverged`. Used for progressive
    /// rendering to determine when all tasks have finished.
    pub fn are_tasks_converged(&self, index: &HdRenderIndex, task_paths: &[SdfPath]) -> bool {
        for path in task_paths {
            if path.is_empty() {
                eprintln!("Warning: Empty task path given to HdEngine::are_tasks_converged()");
                continue;
            }
            if let Some(task) = index.get_task(path) {
                let guard = task.read();
                if !guard.is_converged() {
                    return false;
                }
            } else {
                eprintln!(
                    "Warning: No task at '{}' in render index in HdEngine::are_tasks_converged()",
                    path.as_str()
                );
            }
        }
        true
    }
}

impl Default for HdEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_engine_creation() {
        let engine = HdEngine::new();
        assert_eq!(engine.task_context.len(), 0);
    }

    #[test]
    fn test_task_context_set_get() {
        let mut engine = HdEngine::new();

        let key = Token::new("testData");
        let value = Value::from(42i32);

        // Set
        engine.set_task_context_data(key.clone(), value);
        assert!(engine.get_task_context_data(&key).is_some());

        // Update (same key, new value)
        engine.set_task_context_data(key.clone(), Value::from(99i32));
        assert!(engine.get_task_context_data(&key).is_some());
        // Context should still have 1 entry (updated, not duplicated)
        assert_eq!(engine.task_context.len(), 1);
    }

    #[test]
    fn test_task_context_remove() {
        let mut engine = HdEngine::new();

        let key = Token::new("testData");
        engine.set_task_context_data(key.clone(), Value::from(42i32));
        assert!(engine.get_task_context_data(&key).is_some());

        engine.remove_task_context_data(&key);
        assert!(engine.get_task_context_data(&key).is_none());
    }

    #[test]
    fn test_task_context_clear() {
        let mut engine = HdEngine::new();

        engine.set_task_context_data(Token::new("key1"), Value::from(1i32));
        engine.set_task_context_data(Token::new("key2"), Value::from(2i32));
        assert_eq!(engine.task_context.len(), 2);

        engine.clear_task_context_data();
        assert_eq!(engine.task_context.len(), 0);
    }

    #[test]
    fn test_get_nonexistent_key() {
        let engine = HdEngine::new();
        assert!(
            engine
                .get_task_context_data(&Token::new("missing"))
                .is_none()
        );
    }

    #[test]
    fn test_default_trait() {
        let engine = HdEngine::default();
        assert_eq!(engine.task_context.len(), 0);
    }
}
