
//! Registry for generative procedural plugins.

use super::generative_procedural::HdGpGenerativeProceduralHandle;
use super::generative_procedural_plugin::HdGpGenerativeProceduralPlugin;
use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::sync::Arc;
use parking_lot::RwLock;
use usd_hf::{HfPluginDesc, HfPluginDescVector, HfPluginRegistry, HfPluginRegistryImpl};
use usd_sdf::Path as SdfPath;
use usd_tf::Token as TfToken;

/// Singleton registry for generative procedural plugins.
///
/// Manages discovery and instantiation of generative procedural plugins.
/// Uses the HfPluginRegistry infrastructure with manual registration.
///
/// # Example
///
/// ```ignore
/// use usd_hd_gp::*;
///
/// // Get the singleton
/// let registry = HdGpGenerativeProceduralPluginRegistry::get_instance();
///
/// // Register a plugin
/// registry.register::<MyProceduralPlugin>("My Procedural", 0);
///
/// // Construct a procedural
/// let procedural = registry.construct_procedural(
///     &TfToken::new("MyProceduralPlugin"),
///     &SdfPath::from_string("/Procedural").unwrap(),
/// );
/// ```
/// Type-erased factory: given a prim path, construct a procedural.
type ProceduralFactory =
    Box<dyn Fn(&SdfPath) -> Option<HdGpGenerativeProceduralHandle> + Send + Sync>;

pub struct HdGpGenerativeProceduralPluginRegistry {
    /// Base registry implementation (manages plugin descriptors + instances).
    base: HfPluginRegistryImpl,
    /// Factory map: plugin_id => factory function.
    /// Populated by `register<T>()` for type-safe construct_procedural support.
    factories: HashMap<TfToken, ProceduralFactory>,
    /// Display name -> plugin_id mapping for C++ name-based lookup.
    display_name_to_id: HashMap<String, TfToken>,
}

impl HdGpGenerativeProceduralPluginRegistry {
    /// Returns the singleton registry instance.
    pub fn get_instance() -> Arc<RwLock<Self>> {
        static INSTANCE: Lazy<Arc<RwLock<HdGpGenerativeProceduralPluginRegistry>>> =
            Lazy::new(|| {
                Arc::new(RwLock::new(HdGpGenerativeProceduralPluginRegistry {
                    base: HfPluginRegistryImpl::new(),
                    factories: HashMap::new(),
                    display_name_to_id: HashMap::new(),
                }))
            });

        INSTANCE.clone()
    }

    /// Register a procedural plugin type.
    ///
    /// # Type Parameters
    ///
    /// * `T` - Plugin type implementing HdGpGenerativeProceduralPlugin
    ///
    /// # Arguments
    ///
    /// * `display_name` - Human-readable name for the plugin
    /// * `priority` - Plugin priority (higher = more preferred)
    ///
    /// # Returns
    ///
    /// Token ID for the registered plugin
    pub fn register<T>(&mut self, display_name: &str, priority: i32) -> TfToken
    where
        T: HdGpGenerativeProceduralPlugin + Default + 'static,
    {
        let id_token =
            self.base
                .register::<T>(display_name, priority, Box::new(|| Box::new(T::default())));

        // Also register factory so construct_procedural() works.
        // Matches C++ where GetPlugin(pluginId)->Construct() is used.
        let factory: ProceduralFactory = Box::new(|prim_path: &SdfPath| {
            let plugin = T::default();
            plugin.construct(prim_path)
        });
        self.factories.insert(id_token.clone(), factory);
        self.display_name_to_id
            .insert(display_name.to_string(), id_token.clone());
        id_token
    }

    /// Construct a generative procedural instance.
    ///
    /// Matches C++ `HdGpGenerativeProceduralPluginRegistry::ConstructProcedural`:
    /// 1. Try to find plugin by matching displayName == proceduralTypeName
    /// 2. Fall back to using proceduralTypeName directly as pluginId
    /// 3. Call factory(proceduralPrimPath) registered via `register_with_factory`
    ///
    /// Plugins registered with `register<T>()` alone (without a factory) won't
    /// produce procedurals here. Use `register_with_factory` for full wiring.
    ///
    /// # Arguments
    ///
    /// * `procedural_type_name` - Plugin type or display name token
    /// * `procedural_prim_path` - Scene path for the procedural
    ///
    /// # Returns
    ///
    /// Boxed procedural instance, or None if no factory found for the type
    pub fn construct_procedural(
        &self,
        procedural_type_name: &TfToken,
        procedural_prim_path: &SdfPath,
    ) -> Option<HdGpGenerativeProceduralHandle> {
        // Step 1: Resolve by displayName -> id mapping first (C++ logic),
        // then fall back to treating proceduralTypeName directly as the plugin id.
        let plugin_id = self
            .display_name_to_id
            .get(procedural_type_name.as_str())
            .cloned()
            .unwrap_or_else(|| procedural_type_name.clone());

        // Step 2: Look up and call the factory registered for this plugin id.
        if let Some(factory) = self.factories.get(&plugin_id) {
            return factory(procedural_prim_path);
        }

        None
    }

    /// Register a procedural plugin with an explicit factory function.
    ///
    /// This is the preferred way to register procedurals when `construct_procedural`
    /// needs to work: the factory is called with the prim path to create the procedural.
    pub fn register_with_factory(
        &mut self,
        plugin_id: &str,
        display_name: &str,
        priority: i32,
        factory: Box<dyn Fn(&SdfPath) -> Option<HdGpGenerativeProceduralHandle> + Send + Sync>,
    ) -> TfToken {
        let id_token = TfToken::new(plugin_id);
        self.factories.insert(id_token.clone(), factory);
        // Map display_name -> id so construct_procedural can resolve by name.
        self.display_name_to_id
            .insert(display_name.to_string(), id_token.clone());
        // Also register with base for desc/discovery.
        self.base
            .register_erased(display_name, priority, id_token.clone());
        id_token
    }
}

impl HfPluginRegistry for HdGpGenerativeProceduralPluginRegistry {
    fn get_plugin_descs(&self) -> HfPluginDescVector {
        self.base.get_plugin_descs()
    }

    fn get_plugin_desc(&self, plugin_id: &TfToken) -> Option<HfPluginDesc> {
        self.base.get_plugin_desc(plugin_id)
    }

    fn is_registered(&self, plugin_id: &TfToken) -> bool {
        self.base.is_registered(plugin_id)
    }

    fn get_plugin_id(&self, plugin: &dyn usd_hf::HfPluginBase) -> Option<TfToken> {
        self.base.get_plugin_id(plugin)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generative_procedural::{ChildPrimTypeMap, DependencyMap, HdGpGenerativeProcedural};
    use std::any::Any;
    use usd_hd::scene_index::{HdSceneIndexBase, HdSceneIndexPrim};
    use usd_hf::HfPluginBase;

    // Test procedural
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

    // Test plugin
    #[derive(Default)]
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
    fn test_singleton() {
        let registry1 = HdGpGenerativeProceduralPluginRegistry::get_instance();
        let registry2 = HdGpGenerativeProceduralPluginRegistry::get_instance();

        // Should be the same Arc instance
        assert!(Arc::ptr_eq(&registry1, &registry2));
    }

    #[test]
    fn test_register_plugin() {
        let registry = HdGpGenerativeProceduralPluginRegistry::get_instance();
        let mut reg = registry.write();

        let plugin_id = reg.register::<TestPlugin>("Test Procedural", 0);
        assert!(reg.is_registered(&plugin_id));
    }

    #[test]
    fn test_get_plugin_descs() {
        let registry = HdGpGenerativeProceduralPluginRegistry::get_instance();
        let reg = registry.read();

        let descs = reg.get_plugin_descs();
        // Should at least not crash
        let _ = descs.len();
    }

    // -----------------------------------------------------------------------
    // construct_procedural tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_construct_procedural_returns_none_for_unknown_type() {
        let registry = HdGpGenerativeProceduralPluginRegistry::get_instance();
        let reg = registry.read();

        let unknown = TfToken::new("NonExistentProcedural");
        let path = SdfPath::from_string("/Test").unwrap();
        let result = reg.construct_procedural(&unknown, &path);
        assert!(result.is_none(), "unknown type should return None");
    }

    #[test]
    fn test_construct_procedural_by_plugin_id() {
        // Use a unique id to avoid collision with other tests (singleton registry).
        let registry = HdGpGenerativeProceduralPluginRegistry::get_instance();
        let prim_path = SdfPath::from_string("/MyProcedural").unwrap();

        {
            let mut reg = registry.write();
            reg.register_with_factory(
                "test_procedural_by_id",
                "Test Procedural By Id",
                0,
                Box::new(|path: &SdfPath| {
                    Some(Box::new(TestProcedural {
                        prim_path: path.clone(),
                    }) as HdGpGenerativeProceduralHandle)
                }),
            );
        }

        let reg = registry.read();
        let id_token = TfToken::new("test_procedural_by_id");
        let result = reg.construct_procedural(&id_token, &prim_path);
        assert!(result.is_some(), "should construct by plugin id");

        let procedural = result.unwrap();
        assert_eq!(procedural.get_procedural_prim_path(), &prim_path);
    }

    #[test]
    fn test_construct_procedural_by_display_name() {
        // Look up by display name (C++ ConstructProcedural step 1).
        let registry = HdGpGenerativeProceduralPluginRegistry::get_instance();
        let prim_path = SdfPath::from_string("/NamedProcedural").unwrap();
        let display_name = "My Named Procedural";
        let plugin_id = "test_named_procedural_internal_id";

        {
            let mut reg = registry.write();
            reg.register_with_factory(
                plugin_id,
                display_name,
                0,
                Box::new(|path: &SdfPath| {
                    Some(Box::new(TestProcedural {
                        prim_path: path.clone(),
                    }) as HdGpGenerativeProceduralHandle)
                }),
            );
        }

        let reg = registry.read();
        // Pass display_name as the procedural_type_name (C++ lookup step 1)
        let name_token = TfToken::new(display_name);
        let result = reg.construct_procedural(&name_token, &prim_path);
        assert!(result.is_some(), "should construct by display name");

        let procedural = result.unwrap();
        assert_eq!(procedural.get_procedural_prim_path(), &prim_path);
    }

    #[test]
    fn test_register_with_factory_is_registered() {
        let registry = HdGpGenerativeProceduralPluginRegistry::get_instance();
        {
            let mut reg = registry.write();
            reg.register_with_factory(
                "test_reg_check_id",
                "Test Reg Check",
                5,
                Box::new(|_path| None),
            );
        }
        let reg = registry.read();
        let id = TfToken::new("test_reg_check_id");
        assert!(reg.is_registered(&id));

        // Should also appear in plugin descs
        let descs = reg.get_plugin_descs();
        assert!(descs.iter().any(|d| d.id == id));
    }
}
