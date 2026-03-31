
//! HfPluginDelegateBase - Base trait for Hydra plugin delegate classes.
//!
//! In C++ USD, HdRenderDelegate derives from HfPluginBase via an intermediate
//! HfPluginDelegateBase (conceptual type hierarchy for clarity). This trait
//! separates delegate-specific interface from the generic plugin base.
//!
//! Delegates are plugins that provide rendering/task services to Hydra,
//! as opposed to utility plugins. This distinction matches the C++ hierarchy:
//!   HfPluginBase <- HfPluginDelegateBase <- HdRenderDelegate

use super::plugin_base::HfPluginBase;
use std::any::Any;

/// Base trait for Hydra delegate plugins.
///
/// Extends `HfPluginBase` with delegate-specific identity.
/// All Hydra render delegates and task delegates implement this trait.
///
/// # Type Hierarchy
///
/// ```text
/// HfPluginBase (generic plugin)
///     |
///     +-- HfPluginDelegateBase (Hydra delegate)
///             |
///             +-- HdRenderDelegate (render backend)
/// ```
///
/// # Thread Safety
///
/// Implementations must be Send + Sync, as delegates may be accessed
/// from multiple threads through the plugin registry.
pub trait HfPluginDelegateBase: HfPluginBase {
    /// Returns whether this delegate is a primary (preferred) delegate.
    ///
    /// Primary delegates are presented first in selection UIs.
    /// Default: false.
    fn is_primary_delegate(&self) -> bool {
        false
    }

    /// Provides downcasting support for delegate types.
    fn as_delegate_any(&self) -> &dyn Any;

    /// Provides mutable downcasting for delegate types.
    fn as_delegate_any_mut(&mut self) -> &mut dyn Any;
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockDelegate {
        name: String,
        primary: bool,
    }

    impl MockDelegate {
        fn new(name: &str, primary: bool) -> Self {
            Self {
                name: name.to_string(),
                primary,
            }
        }
    }

    impl HfPluginBase for MockDelegate {
        fn type_name(&self) -> &'static str {
            "MockDelegate"
        }

        fn as_any(&self) -> &dyn Any {
            self
        }

        fn as_any_mut(&mut self) -> &mut dyn Any {
            self
        }
    }

    impl HfPluginDelegateBase for MockDelegate {
        fn is_primary_delegate(&self) -> bool {
            self.primary
        }

        fn as_delegate_any(&self) -> &dyn Any {
            self
        }

        fn as_delegate_any_mut(&mut self) -> &mut dyn Any {
            self
        }
    }

    #[test]
    fn test_delegate_base_trait() {
        let delegate = MockDelegate::new("storm", false);
        assert_eq!(delegate.type_name(), "MockDelegate");
        assert!(!delegate.is_primary_delegate());
    }

    #[test]
    fn test_primary_delegate() {
        let delegate = MockDelegate::new("storm", true);
        assert!(delegate.is_primary_delegate());
    }

    #[test]
    fn test_delegate_downcasting() {
        let delegate = MockDelegate::new("test", false);
        let base: &dyn HfPluginDelegateBase = &delegate;

        let concrete = base.as_delegate_any().downcast_ref::<MockDelegate>();
        assert!(concrete.is_some());
        assert_eq!(concrete.unwrap().name, "test");
    }

    #[test]
    fn test_delegate_as_plugin_base() {
        // Delegates must also be usable as HfPluginBase
        let delegate = MockDelegate::new("test", false);
        let plugin: &dyn HfPluginBase = &delegate;
        assert_eq!(plugin.type_name(), "MockDelegate");
    }
}
