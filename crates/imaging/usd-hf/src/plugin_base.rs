
//! HfPluginBase - Base trait for all Hydra plugin classes.
//!
//! This trait provides the polymorphic interface for the plugin registry.
//! All Hydra plugins (render delegates, etc.) must implement this trait.

use std::any::Any;

/// Base trait for all Hydra plugin classes.
///
/// This trait provides minimal functionality to serve as a polymorphic type
/// for the plugin registry. Derived types should implement their specific
/// plugin APIs as separate traits.
///
/// # Thread Safety
///
/// Implementations must be Send + Sync as plugins may be accessed from
/// multiple threads through the registry.
pub trait HfPluginBase: Send + Sync {
    /// Returns the type name of this plugin for debugging and registry lookup.
    fn type_name(&self) -> &'static str;

    /// Provides downcasting support via Any trait.
    ///
    /// This allows the registry to safely downcast to concrete plugin types.
    fn as_any(&self) -> &dyn Any;

    /// Provides mutable downcasting support via Any trait.
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

#[cfg(test)]
mod tests {
    use super::*;

    // Mock plugin for testing
    struct MockPlugin {
        name: String,
    }

    impl MockPlugin {
        fn new(name: &str) -> Self {
            Self {
                name: name.to_string(),
            }
        }
    }

    impl HfPluginBase for MockPlugin {
        fn type_name(&self) -> &'static str {
            "MockPlugin"
        }

        fn as_any(&self) -> &dyn Any {
            self
        }

        fn as_any_mut(&mut self) -> &mut dyn Any {
            self
        }
    }

    #[test]
    fn test_plugin_base_trait() {
        let plugin = MockPlugin::new("test");
        assert_eq!(plugin.type_name(), "MockPlugin");
    }

    #[test]
    fn test_plugin_downcasting() {
        let plugin = MockPlugin::new("test");
        let base: &dyn HfPluginBase = &plugin;

        // Test successful downcast
        let concrete = base.as_any().downcast_ref::<MockPlugin>();
        assert!(concrete.is_some());
        assert_eq!(concrete.unwrap().name, "test");
    }

    #[test]
    fn test_plugin_trait_object() {
        let plugin: Box<dyn HfPluginBase> = Box::new(MockPlugin::new("boxed"));
        assert_eq!(plugin.type_name(), "MockPlugin");
    }
}
