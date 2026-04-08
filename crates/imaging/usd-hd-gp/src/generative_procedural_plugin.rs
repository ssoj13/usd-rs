//! Plugin interface for generative procedurals.

use super::generative_procedural::HdGpGenerativeProceduralHandle;
use usd_hf::HfPluginBase;
use usd_sdf::Path as SdfPath;

/// Plugin trait for discovering and constructing generative procedurals.
///
/// Plugins implement this trait to register procedural types with the
/// HdGpGenerativeProceduralPluginRegistry. The plugin system uses the
/// HfPluginBase infrastructure for discovery and lifecycle management.
///
/// # Example
///
/// ```ignore
/// use usd_hd_gp::*;
///
/// struct MyProceduralPlugin;
///
/// impl HfPluginBase for MyProceduralPlugin {
///     fn type_name(&self) -> &'static str {
///         "MyProceduralPlugin"
///     }
///     
///     fn as_any(&self) -> &dyn std::any::Any {
///         self
///     }
///     
///     fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
///         self
///     }
/// }
///
/// impl HdGpGenerativeProceduralPlugin for MyProceduralPlugin {
///     fn construct(&self, procedural_prim_path: &SdfPath) -> Option<HdGpGenerativeProceduralHandle> {
///         Some(Box::new(MyProcedural::new(procedural_prim_path.clone())))
///     }
/// }
/// ```
pub trait HdGpGenerativeProceduralPlugin: HfPluginBase {
    /// Construct a generative procedural instance.
    ///
    /// Called by the registry when a procedural prim needs to be instantiated.
    /// Returns None if this plugin cannot handle the requested procedural.
    ///
    /// # Arguments
    ///
    /// * `procedural_prim_path` - Scene path for the procedural prim
    ///
    /// # Returns
    ///
    /// Boxed procedural instance, or None if construction fails
    fn construct(&self, procedural_prim_path: &SdfPath) -> Option<HdGpGenerativeProceduralHandle>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generative_procedural::{ChildPrimTypeMap, DependencyMap, HdGpGenerativeProcedural};
    use std::any::Any;
    use usd_hd::scene_index::{HdSceneIndexBase, HdSceneIndexPrim};

    // Test procedural implementation
    struct TestProcedural {
        prim_path: SdfPath,
    }

    impl HdGpGenerativeProcedural for TestProcedural {
        fn get_procedural_prim_path(&self) -> &SdfPath {
            &self.prim_path
        }

        fn update_dependencies(&mut self, _input_scene: &dyn HdSceneIndexBase) -> DependencyMap {
            DependencyMap::new()
        }

        fn update(
            &mut self,
            _input_scene: &dyn HdSceneIndexBase,
            _previous_result: &ChildPrimTypeMap,
            _dirtied_dependencies: &DependencyMap,
            _output_dirtied_prims: &mut Vec<usd_hd::scene_index::DirtiedPrimEntry>,
        ) -> ChildPrimTypeMap {
            ChildPrimTypeMap::new()
        }

        fn get_child_prim(
            &self,
            _input_scene: &dyn HdSceneIndexBase,
            _child_prim_path: &SdfPath,
        ) -> HdSceneIndexPrim {
            HdSceneIndexPrim::default()
        }
    }

    // Test plugin implementation
    struct TestPlugin;

    impl HfPluginBase for TestPlugin {
        fn type_name(&self) -> &'static str {
            "TestProceduralPlugin"
        }

        fn as_any(&self) -> &dyn Any {
            self
        }

        fn as_any_mut(&mut self) -> &mut dyn Any {
            self
        }
    }

    impl HdGpGenerativeProceduralPlugin for TestPlugin {
        fn construct(
            &self,
            procedural_prim_path: &SdfPath,
        ) -> Option<HdGpGenerativeProceduralHandle> {
            Some(Box::new(TestProcedural {
                prim_path: procedural_prim_path.clone(),
            }))
        }
    }

    #[test]
    fn test_plugin_construct() {
        let plugin = TestPlugin;
        let path = SdfPath::from_string("/Test").unwrap();

        let procedural = plugin.construct(&path);
        assert!(procedural.is_some());

        if let Some(proc) = procedural {
            assert_eq!(proc.get_procedural_prim_path(), &path);
        }
    }

    #[test]
    fn test_plugin_type_name() {
        let plugin = TestPlugin;
        assert_eq!(plugin.type_name(), "TestProceduralPlugin");
    }
}
