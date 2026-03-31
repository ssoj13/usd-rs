
//! Hf (Hydra Foundation) - Plugin system foundation for Hydra render delegates.
//!
//! This module provides the base plugin architecture that Hydra uses for
//! extensibility. It includes:
//!
//! - [`HfPluginBase`] - Base trait for all Hydra plugins
//! - [`HfPluginDesc`] - Plugin descriptor with metadata
//! - [`HfPluginRegistry`] - Plugin registry for discovery and management
//! - Performance logging macros (no-op in Rust)
//! - Diagnostic utilities for validation warnings
//!
//! # Architecture
//!
//! The plugin system is designed for extensibility:
//!
//! 1. **Plugin Base**: All plugins implement `HfPluginBase` trait
//! 2. **Registration**: Plugins register with a registry providing factory, priority
//! 3. **Discovery**: Registries maintain ordered lists by priority
//! 4. **Lifecycle**: Ref counting manages plugin instance lifetime
//! 5. **Type Safety**: Downcasting via `Any` trait for concrete access
//!
//! # Example
//!
//! ```
//! use usd_hf::{HfPluginBase, HfPluginRegistryImpl};
//! use std::any::Any;
//!
//! // Define a custom plugin
//! struct MyRenderDelegate {
//!     name: String,
//! }
//!
//! impl HfPluginBase for MyRenderDelegate {
//!     fn type_name(&self) -> &'static str {
//!         "MyRenderDelegate"
//!     }
//!
//!     fn as_any(&self) -> &dyn Any {
//!         self
//!     }
//!
//!     fn as_any_mut(&mut self) -> &mut dyn Any {
//!         self
//!     }
//! }
//!
//! // Register the plugin
//! let registry = HfPluginRegistryImpl::new();
//! let id = registry.register::<MyRenderDelegate>(
//!     "My Render Delegate",
//!     100,
//!     Box::new(|| Box::new(MyRenderDelegate { name: "test".into() })),
//! );
//!
//! // Get plugin instance
//! let plugin = registry.get_plugin(&id);
//! assert!(plugin.is_some());
//! ```
//!
//! # Performance
//!
//! The perf_log module provides no-op macros in Rust. For actual profiling,
//! use standard Rust tools like:
//!
//! - `cargo-flamegraph` for flamegraphs
//! - `perf` for Linux profiling
//! - `instruments` for macOS profiling
//! - `tracing` crate for structured logging

pub mod diagnostic;
pub mod perf_log;
pub mod plugin_base;
pub mod plugin_delegate_base;
pub mod plugin_desc;
pub mod plugin_entry;
pub mod plugin_registry;

// Re-export main types
pub use plugin_base::HfPluginBase;
pub use plugin_delegate_base::HfPluginDelegateBase;
pub use plugin_desc::{HfPluginDesc, HfPluginDescVector};
pub use plugin_registry::{
    HfPluginAutoEntry, HfPluginRegistry, HfPluginRegistryImpl, register_hf_plugin_auto,
};

// NOTE: #[macro_export] macros are already at crate root.
// Do not re-export them to avoid E0255.

#[cfg(test)]
mod tests {
    use super::*;
    use std::any::Any;

    struct TestPlugin {
        value: i32,
    }

    impl HfPluginBase for TestPlugin {
        fn type_name(&self) -> &'static str {
            std::any::type_name::<Self>()
        }

        fn as_any(&self) -> &dyn Any {
            self
        }

        fn as_any_mut(&mut self) -> &mut dyn Any {
            self
        }
    }

    #[test]
    fn test_module_exports() {
        // Test that all main types are accessible
        let _desc = HfPluginDesc::new(usd_tf::Token::new("test"), "Test".to_string(), 100);

        let _registry = HfPluginRegistryImpl::new();
    }

    #[test]
    fn test_full_plugin_workflow() {
        let registry = HfPluginRegistryImpl::new();

        // Register plugin
        let id = registry.register::<TestPlugin>(
            "Test Plugin",
            100,
            Box::new(|| Box::new(TestPlugin { value: 42 })),
        );

        // Check registration
        assert!(registry.is_registered(&id));

        // Get descriptor
        let desc = registry.get_plugin_desc(&id).unwrap();
        assert_eq!(desc.display_name, "Test Plugin");
        assert_eq!(desc.priority, 100);

        // Get plugin instance
        let plugin_lock = registry.get_plugin(&id).unwrap();
        let plugin_guard = plugin_lock.read().expect("plugin lock poisoned");
        let plugin = plugin_guard.as_ref().unwrap();

        // Downcast to concrete type
        let concrete = plugin.as_any().downcast_ref::<TestPlugin>().unwrap();
        assert_eq!(concrete.value, 42);

        // Release plugin
        drop(plugin_guard);
        drop(plugin_lock);
        registry.release_plugin(&id);
    }

    #[test]
    fn test_perf_macros_available() {
        hf_malloc_tag_function!();
        hf_malloc_tag!("test");
        hf_trace_function_scope!("test");
    }

    #[test]
    fn test_diagnostic_macros_available() {
        struct FakePath(&'static str);
        impl FakePath {
            fn as_str(&self) -> &str {
                self.0
            }
        }

        let path = FakePath("/test");
        hf_validation_warn!(path, "test warning");
    }

    #[allow(dead_code)]
    struct HighPriorityPlugin(i32);
    #[allow(dead_code)]
    struct LowPriorityPlugin(i32);

    impl HfPluginBase for HighPriorityPlugin {
        fn type_name(&self) -> &'static str {
            std::any::type_name::<Self>()
        }
        fn as_any(&self) -> &dyn Any {
            self
        }
        fn as_any_mut(&mut self) -> &mut dyn Any {
            self
        }
    }

    impl HfPluginBase for LowPriorityPlugin {
        fn type_name(&self) -> &'static str {
            std::any::type_name::<Self>()
        }
        fn as_any(&self) -> &dyn Any {
            self
        }
        fn as_any_mut(&mut self) -> &mut dyn Any {
            self
        }
    }

    #[test]
    fn test_plugin_ordering_by_priority() {
        let registry = HfPluginRegistryImpl::new();

        // Register plugins with different priorities and different types
        registry.register::<LowPriorityPlugin>(
            "Low Priority",
            10,
            Box::new(|| Box::new(LowPriorityPlugin(1))),
        );

        registry.register::<HighPriorityPlugin>(
            "High Priority",
            100,
            Box::new(|| Box::new(HighPriorityPlugin(2))),
        );

        let descs = registry.get_plugin_descs();
        assert_eq!(descs.len(), 2);

        // C++ sorts ascending: lower numeric priority = sorts first (index 0).
        // priority=10 (Low) < priority=100 (High), so Low comes first.
        assert_eq!(descs[0].display_name, "Low Priority");
        assert_eq!(descs[1].display_name, "High Priority");
    }
}
