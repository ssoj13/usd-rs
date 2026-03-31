//! Interface factory types for plugin-provided singleton interfaces.
//!
//! Port of pxr/base/plug/interfaceFactory.h
//!
//! Provides `PlugInterfaceFactory` (analogous to `Plug_InterfaceFactory::Base`)
//! and `SingletonFactory<T>` (analogous to `Plug_InterfaceFactory::SingletonFactory`)
//! for registering singleton implementations of abstract interface types with
//! the TfType system.

use std::any::Any;
use std::sync::Arc;
use std::sync::OnceLock;
use usd_tf::FactoryBase;

/// Trait for plug interface factories.
///
/// Matches C++ `Plug_InterfaceFactory::Base`. Implementors return a raw
/// const pointer to the singleton implementation. The pointer must remain
/// valid for the lifetime of the process.
pub trait PlugInterfaceFactory: FactoryBase {
    /// Returns a raw pointer to the singleton implementation.
    ///
    /// Matches C++ `Plug_InterfaceFactory::Base::New()`.
    fn new_instance(&self) -> *const ();
}

/// A factory that returns a single lazily-initialized instance of `T`.
///
/// Matches C++ `Plug_InterfaceFactory::SingletonFactory<Interface, Implementation>`.
/// The instance is created on the first call to `new_instance()` and lives for
/// the lifetime of the program (stored in a `'static OnceLock`).
///
/// # Safety
///
/// The returned raw pointer points to a `'static` value stored inside the
/// `OnceLock`. It is safe to dereference for the lifetime of the process.
pub struct SingletonFactory<T: Send + Sync + 'static> {
    instance: OnceLock<T>,
    /// Called exactly once to construct the singleton instance.
    init: fn() -> T,
}

impl<T: Send + Sync + 'static> SingletonFactory<T> {
    /// Creates a new `SingletonFactory` with the given initializer function.
    pub const fn new(init: fn() -> T) -> Self {
        Self {
            instance: OnceLock::new(),
            init,
        }
    }
}

impl<T: Send + Sync + 'static> PlugInterfaceFactory for SingletonFactory<T> {
    fn new_instance(&self) -> *const () {
        let ptr: *const T = self.instance.get_or_init(self.init);
        ptr as *const ()
    }
}

// FactoryBase impl so SingletonFactory can be stored in TfType.
impl<T: Send + Sync + 'static> FactoryBase for SingletonFactory<T> {
    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Registers an interface type with TfType, associating it with a singleton
/// factory that returns an instance of the given implementation type.
///
/// Matches the C++ macro `PLUG_REGISTER_INTERFACE_SINGLETON_TYPE`.
///
/// Call this during program initialization (e.g., from a
/// `tf_registry_function`-equivalent) to make a concrete `Implementation`
/// available for a given abstract `Interface` type name.
///
/// The `interface_name` must match the canonical type name used by
/// `TfType::find_by_name` (typically the fully-qualified Rust path).
pub fn register_interface_singleton<Implementation: Send + Sync + 'static>(
    interface_name: &str,
    init: fn() -> Implementation,
) {
    let tf_type = usd_tf::TfType::find_by_name(interface_name);
    let factory = Arc::new(SingletonFactory::new(init));
    tf_type.set_factory(factory);
}

#[cfg(test)]
mod tests {
    use super::*;

    struct FakeImpl {
        value: i32,
    }

    #[test]
    fn test_singleton_factory_returns_same_pointer() {
        let factory = SingletonFactory::new(|| FakeImpl { value: 42 });
        let ptr1 = factory.new_instance();
        let ptr2 = factory.new_instance();
        // Must be the exact same singleton pointer on both calls.
        assert_eq!(ptr1, ptr2);
    }

    #[test]
    fn test_singleton_factory_value_accessible() {
        let factory = SingletonFactory::new(|| FakeImpl { value: 99 });
        let ptr = factory.new_instance() as *const FakeImpl;
        // SAFETY: pointer comes from our OnceLock-backed singleton which is
        // valid for the lifetime of the factory.
        #[allow(unsafe_code)]
        let value = unsafe { (*ptr).value };
        assert_eq!(value, 99);
    }

    #[test]
    fn test_factory_base_downcast() {
        let factory: Arc<dyn FactoryBase> =
            Arc::new(SingletonFactory::new(|| FakeImpl { value: 7 }));
        // Must be downtrodden back to SingletonFactory<FakeImpl>.
        assert!(factory.as_any().is::<SingletonFactory<FakeImpl>>());
    }
}
