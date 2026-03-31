
//! HdGp (Hydra Generative Procedurals) - Procedural primitive generation for Hydra.
//!
//! This module provides support for generative procedural primitives in Hydra.
//! Procedurals can generate dynamic scene content based on input scene data,
//! with full dependency tracking and incremental updates.
//!
//! # Architecture
//!
//! - [`HdGpGenerativeProcedural`] - Base trait for procedural implementations
//! - [`HdGpGenerativeProceduralPlugin`] - Plugin interface for procedural discovery
//! - [`HdGpGenerativeProceduralPluginRegistry`] - Registry for procedural plugins
//! - [`HdGpGenerativeProceduralResolvingSceneIndex`] - Scene index that resolves procedurals
//! - [`HdGpSceneIndexPlugin`] - Scene index plugin integration
//!
//! # Workflow
//!
//! 1. Procedurals implement the `HdGpGenerativeProcedural` trait
//! 2. Plugins register procedurals with the registry
//! 3. Scene index resolves procedural prims and generates children
//! 4. Dependencies are tracked for incremental updates
//! 5. Async support for long-running procedurals
//!
//! # Example
//!
//! ```ignore
//! use usd_hd_gp::*;
//!
//! // Define a custom procedural
//! struct MyProcedural {
//!     prim_path: SdfPath,
//! }
//!
//! impl HdGpGenerativeProcedural for MyProcedural {
//!     fn update_dependencies(&mut self, input_scene: &dyn HdSceneIndexBase) -> DependencyMap {
//!         // Declare dependencies on input scene data
//!         DependencyMap::new()
//!     }
//!
//!     fn update(
//!         &mut self,
//!         input_scene: &dyn HdSceneIndexBase,
//!         previous_result: &ChildPrimTypeMap,
//!         dirtied_dependencies: &DependencyMap,
//!         output_dirtied_prims: &mut DirtiedPrimEntries,
//!     ) -> ChildPrimTypeMap {
//!         // Generate child prims
//!         ChildPrimTypeMap::new()
//!     }
//!
//!     fn get_child_prim(
//!         &self,
//!         input_scene: &dyn HdSceneIndexBase,
//!         child_prim_path: &SdfPath,
//!     ) -> HdSceneIndexPrim {
//!         // Return child prim data
//!         HdSceneIndexPrim::default()
//!     }
//! }
//! ```

pub mod generative_procedural;
pub mod generative_procedural_filtering_scene_index;
pub mod generative_procedural_plugin;
pub mod generative_procedural_plugin_registry;
pub mod generative_procedural_resolving_scene_index;
pub mod scene_index_plugin;

// Re-export core types
pub use generative_procedural::{
    AsyncState, ChildPrimTypeMap, DependencyMap, HdGpGenerativeProcedural,
    tokens as procedural_tokens,
};
pub use generative_procedural_filtering_scene_index::{
    HdGpGenerativeProceduralFilteringSceneIndex, HdGpGenerativeProceduralFilteringSceneIndexHandle,
};
pub use generative_procedural_plugin::HdGpGenerativeProceduralPlugin;
pub use generative_procedural_plugin_registry::HdGpGenerativeProceduralPluginRegistry;
pub use generative_procedural_resolving_scene_index::{
    HdGpGenerativeProceduralResolvingSceneIndex, HdGpGenerativeProceduralResolvingSceneIndexHandle,
};
pub use scene_index_plugin::HdGpSceneIndexPlugin;

#[cfg(test)]
mod tests {
    use super::*;
    use usd_hd::scene_index::HdSceneIndexPrim;
    use usd_sdf::Path as SdfPath;

    /// Test procedural stub
    struct TestProcedural {
        prim_path: SdfPath,
    }

    impl HdGpGenerativeProcedural for TestProcedural {
        fn get_procedural_prim_path(&self) -> &SdfPath {
            &self.prim_path
        }

        fn update_dependencies(
            &mut self,
            _input_scene: &dyn usd_hd::scene_index::HdSceneIndexBase,
        ) -> DependencyMap {
            DependencyMap::new()
        }

        fn update(
            &mut self,
            _input_scene: &dyn usd_hd::scene_index::HdSceneIndexBase,
            _previous_result: &ChildPrimTypeMap,
            _dirtied_dependencies: &DependencyMap,
            _output_dirtied_prims: &mut Vec<usd_hd::scene_index::DirtiedPrimEntry>,
        ) -> ChildPrimTypeMap {
            ChildPrimTypeMap::new()
        }

        fn get_child_prim(
            &self,
            _input_scene: &dyn usd_hd::scene_index::HdSceneIndexBase,
            _child_prim_path: &SdfPath,
        ) -> HdSceneIndexPrim {
            HdSceneIndexPrim::default()
        }
    }

    #[test]
    fn test_procedural_creation() {
        let path = SdfPath::from_string("/Procedural").unwrap();
        let proc = TestProcedural {
            prim_path: path.clone(),
        };
        assert_eq!(proc.get_procedural_prim_path(), &path);
    }

    #[test]
    fn test_async_state_values() {
        // Verify enum values
        assert_eq!(AsyncState::Continuing as u32, 0);
        assert_eq!(AsyncState::Finished as u32, 1);
        assert_eq!(AsyncState::ContinuingWithNewChanges as u32, 2);
        assert_eq!(AsyncState::FinishedWithNewChanges as u32, 3);
    }

    #[test]
    fn test_tokens() {
        assert_eq!(
            procedural_tokens::GENERATIVE_PROCEDURAL.as_str(),
            "hydraGenerativeProcedural"
        );
        assert_eq!(
            procedural_tokens::RESOLVED_GENERATIVE_PROCEDURAL.as_str(),
            "resolvedHydraGenerativeProcedural"
        );
        assert_eq!(
            procedural_tokens::SKIPPED_GENERATIVE_PROCEDURAL.as_str(),
            "skippedHydraGenerativeProcedural"
        );
        assert_eq!(
            procedural_tokens::PROCEDURAL_TYPE.as_str(),
            "hdGp:proceduralType"
        );
        assert_eq!(procedural_tokens::ANY_PROCEDURAL_TYPE.as_str(), "*");
    }
}
