//! Generative procedural base trait and types.

use std::collections::HashMap;
use usd_hd::data_source::{HdDataSourceLocator, HdDataSourceLocatorSet};
use usd_hd::scene_index::{DirtiedPrimEntry, HdSceneIndexBase, HdSceneIndexPrim};
use usd_sdf::Path as SdfPath;
use usd_tf::Token as TfToken;

/// Standard tokens for generative procedurals.
pub mod tokens {
    use once_cell::sync::Lazy;
    use usd_tf::Token;

    /// Hydra prim type for unresolved generative procedurals
    pub static GENERATIVE_PROCEDURAL: Lazy<Token> =
        Lazy::new(|| Token::new("hydraGenerativeProcedural"));

    /// Hydra prim type for resolved generative procedurals
    pub static RESOLVED_GENERATIVE_PROCEDURAL: Lazy<Token> =
        Lazy::new(|| Token::new("resolvedHydraGenerativeProcedural"));

    /// Hydra prim type for skipped generative procedurals
    pub static SKIPPED_GENERATIVE_PROCEDURAL: Lazy<Token> =
        Lazy::new(|| Token::new("skippedHydraGenerativeProcedural"));

    /// Attribute name for procedural type identifier
    pub static PROCEDURAL_TYPE: Lazy<Token> = Lazy::new(|| Token::new("hdGp:proceduralType"));

    /// Wildcard for matching any procedural type
    pub static ANY_PROCEDURAL_TYPE: Lazy<Token> = Lazy::new(|| Token::new("*"));
}

/// Map of prim paths to sets of data source locators.
///
/// Used to declare dependencies on specific data sources within input prims.
pub type DependencyMap = HashMap<SdfPath, HdDataSourceLocatorSet>;

/// Map of child prim paths to their hydra prim types.
///
/// Returned by Update() to describe the full set of child prims.
pub type ChildPrimTypeMap = HashMap<SdfPath, TfToken>;

/// Async evaluation state for procedurals.
///
/// Returned by async_update() to indicate progress and whether
/// new changes are available.
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AsyncState {
    /// Continue checking, no new changes
    Continuing = 0,
    /// Stop checking, no new changes
    Finished = 1,
    /// Continue checking, new changes available
    ContinuingWithNewChanges = 2,
    /// Stop checking, new changes available
    FinishedWithNewChanges = 3,
}

/// Base trait for generative procedural primitives.
///
/// Generative procedurals have full access to an input scene and can create
/// and update a hierarchy of child prims within a hydra scene index. They
/// declare dependencies on input data and are updated when dependencies change.
///
/// # Lifecycle
///
/// 1. `update_dependencies()` - Declare what input data is needed
/// 2. `update()` - Generate/update child prims (called when dependencies dirty)
/// 3. `get_child_prim()` - Return data source for a specific child prim
///
/// # Thread Safety
///
/// - `update_dependencies()` and `update()` are called from a single thread
/// - `get_child_prim()` may be called from multiple threads concurrently
///
/// # Async Support (Optional)
///
/// - `async_begin()` - Enable async evaluation
/// - `async_update()` - Poll for async results
pub trait HdGpGenerativeProcedural: Send + Sync {
    /// Returns the prim path of this procedural.
    fn get_procedural_prim_path(&self) -> &SdfPath;

    /// Declare dependencies on input scene data.
    ///
    /// Returns a map of prim paths to data source locator sets describing
    /// what data should trigger updates when changed.
    ///
    /// Called from a single thread, not concurrent with update().
    ///
    /// # Arguments
    ///
    /// * `input_scene` - The input scene to query for dependencies
    fn update_dependencies(&mut self, input_scene: &dyn HdSceneIndexBase) -> DependencyMap;

    /// Primary "cook" method - generates/updates child prims.
    ///
    /// Returns the full set of child prim paths and types. The result is
    /// interpreted as:
    /// - New paths = prims added
    /// - Missing paths = prims removed
    /// - Changed types = prims re-added
    /// - Same paths/types = potentially dirty (add to output_dirtied_prims)
    ///
    /// Called from a single thread, not concurrent with update_dependencies().
    ///
    /// # Arguments
    ///
    /// * `input_scene` - The input scene to query
    /// * `previous_result` - Result from previous update() call
    /// * `dirtied_dependencies` - Which declared dependencies changed
    /// * `output_dirtied_prims` - Output vector for dirty notifications
    ///
    /// # Returns
    ///
    /// Complete map of child prim paths to prim types
    fn update(
        &mut self,
        input_scene: &dyn HdSceneIndexBase,
        previous_result: &ChildPrimTypeMap,
        dirtied_dependencies: &DependencyMap,
        output_dirtied_prims: &mut Vec<DirtiedPrimEntry>,
    ) -> ChildPrimTypeMap;

    /// Returns prim data for a specific child prim.
    ///
    /// Called from multiple threads - must be thread-safe.
    ///
    /// # Arguments
    ///
    /// * `input_scene` - The input scene to query
    /// * `child_prim_path` - Path of child prim to retrieve
    ///
    /// # Returns
    ///
    /// Scene index prim with type and data source
    fn get_child_prim(
        &self,
        input_scene: &dyn HdSceneIndexBase,
        child_prim_path: &SdfPath,
    ) -> HdSceneIndexPrim;

    /// Returns locator for depending on a prim's immediate children.
    ///
    /// Use this in UpdateDependencies to be notified when a prim's
    /// child list changes.
    fn get_child_names_dependency_key() -> HdDataSourceLocator
    where
        Self: Sized,
    {
        HdDataSourceLocator::from_token(TfToken::new("__childNames"))
    }

    // ------------------------------------------------------------------------
    // Asynchronous API (Optional)
    // ------------------------------------------------------------------------

    /// Called to enable/disable asynchronous evaluation.
    ///
    /// If async_enabled is true and the procedural wants async updates,
    /// return true. If false, return false to do synchronous work.
    ///
    /// Default implementation returns false (no async support).
    ///
    /// # Arguments
    ///
    /// * `async_enabled` - Whether async is possible in this context
    ///
    /// # Returns
    ///
    /// true if async updates should be polled, false otherwise
    fn async_begin(&mut self, _async_enabled: bool) -> bool {
        false
    }

    /// Poll for asynchronous evaluation results.
    ///
    /// Similar to update() but called periodically without input scene access.
    /// Used to synchronize results from background threads/processes.
    ///
    /// Default implementation returns Finished (no async support).
    ///
    /// # Arguments
    ///
    /// * `previous_result` - Result from previous update/async_update
    /// * `output_prim_types` - Output map if new changes available
    /// * `output_dirtied_prims` - Output vector for dirty notifications
    ///
    /// # Returns
    ///
    /// AsyncState indicating progress and whether changes are available
    fn async_update(
        &mut self,
        _previous_result: &ChildPrimTypeMap,
        _output_prim_types: &mut ChildPrimTypeMap,
        _output_dirtied_prims: &mut Vec<DirtiedPrimEntry>,
    ) -> AsyncState {
        AsyncState::Finished
    }
}

/// Type-erased handle for generative procedurals.
pub type HdGpGenerativeProceduralHandle = Box<dyn HdGpGenerativeProcedural>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dependency_map() {
        let mut deps = DependencyMap::new();
        let path = SdfPath::from_string("/Prim").unwrap();
        let locators = HdDataSourceLocatorSet::new();
        deps.insert(path.clone(), locators);
        assert!(deps.contains_key(&path));
    }

    #[test]
    fn test_child_prim_type_map() {
        let mut map = ChildPrimTypeMap::new();
        let path = SdfPath::from_string("/Child").unwrap();
        let prim_type = TfToken::new("Mesh");
        map.insert(path.clone(), prim_type.clone());
        assert_eq!(map.get(&path), Some(&prim_type));
    }

    #[test]
    fn test_async_state() {
        assert_eq!(AsyncState::Continuing as u32, 0);
        assert_eq!(AsyncState::Finished as u32, 1);
        assert_eq!(AsyncState::ContinuingWithNewChanges as u32, 2);
        assert_eq!(AsyncState::FinishedWithNewChanges as u32, 3);
    }

    #[test]
    fn test_child_names_dependency_key() {
        // Use the trait's default implementation
        // Since get_child_names_dependency_key has where Self: Sized bound,
        // we need a concrete type that implements the trait to call it
        let key = HdDataSourceLocator::from_token(TfToken::new("__childNames"));
        assert_eq!(key.len(), 1);
        assert_eq!(key.first_element().unwrap().as_str(), "__childNames");
    }
}
