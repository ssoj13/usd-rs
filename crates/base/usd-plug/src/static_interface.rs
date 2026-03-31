//! Lazy-loaded plugin interface pointers.
//!
//! Port of pxr/base/plug/staticInterface.h / staticInterface.cpp
//!
//! `PlugStaticInterface<T>` provides zero-cost access to a plugin-provided
//! singleton interface without requiring a direct link against the plugin.
//! The interface pointer is resolved on first access by consulting TfType
//! and the plugin registry.
//!
//! # Usage
//!
//! ```ignore
//! use usd_plug::static_interface::PlugStaticInterface;
//!
//! static MY_IFACE: PlugStaticInterface<MyConcreteType> =
//!     PlugStaticInterface::new("MyInterface");
//!
//! if let Some(iface) = MY_IFACE.get() {
//!     iface.do_something();
//! }
//! ```
//!
//! # Restrictions (matching C++)
//!
//! Only declare `PlugStaticInterface` as:
//!   - a file-scope `static`
//!   - a `static` member of a struct
//!   - a `static` inside a function body
//!
//! Do **not** create it as a local variable, struct field, or function argument.

use std::sync::OnceLock;

/// Lazy-loaded pointer to a plugin-provided singleton interface.
///
/// Matches C++ `PlugStaticInterface<Interface>` + `Plug_StaticInterfaceBase`.
///
/// The type parameter `T` is the concrete interface type. On first call to
/// `get()` the pointer is resolved via TfType and cached. Subsequent calls
/// return the cached pointer immediately with no locking overhead.
///
/// The inner `OnceLock` stores `Option<*const ()>`:
///   - `None` means initialization was attempted but failed (no plugin).
///   - `Some(ptr)` means the singleton is available.
pub struct PlugStaticInterface<T: 'static> {
    // We store *const () rather than *const T so the struct stays FFI-friendly
    // and avoids variance issues. The pointer is reinterpreted on access.
    inner: OnceLock<Option<*const ()>>,
    /// The canonical TfType name for the interface (used for factory lookup).
    type_name: &'static str,
    _marker: std::marker::PhantomData<*const T>,
}

// SAFETY: The stored raw pointer always points to a 'static singleton inside
// a SingletonFactory. It is never mutated after initialization and OnceLock
// provides single-writer guarantees during init.
#[allow(unsafe_code)]
unsafe impl<T: 'static> Send for PlugStaticInterface<T> {}
#[allow(unsafe_code)]
unsafe impl<T: 'static> Sync for PlugStaticInterface<T> {}

impl<T: 'static> PlugStaticInterface<T> {
    /// Creates an uninitialized interface wrapper.
    ///
    /// `type_name` must be the canonical name registered with TfType (i.e. the
    /// same string passed to `register_interface_singleton` or `TfType::define`).
    ///
    /// This is a `const fn` so it can be used in `static` initializers.
    pub const fn new(type_name: &'static str) -> Self {
        Self {
            inner: OnceLock::new(),
            type_name,
            _marker: std::marker::PhantomData,
        }
    }

    /// Returns whether initialization has been attempted (regardless of
    /// outcome).
    ///
    /// Matches C++ `Plug_StaticInterfaceBase::IsInitialized()`.
    pub fn is_initialized(&self) -> bool {
        self.inner.get().is_some()
    }

    /// Returns the interface pointer if the plugin is available, or `None`.
    ///
    /// On first call this performs TfType lookup, plugin loading, and factory
    /// invocation. Subsequent calls return the cached pointer immediately.
    ///
    /// Matches C++ `PlugStaticInterface<Interface>::Get()` and
    /// `Plug_StaticInterfaceBase::_LoadAndInstantiate`.
    pub fn get(&self) -> Option<&'static T> {
        let opt_ptr = self.inner.get_or_init(|| self.load_and_instantiate());
        // SAFETY: The pointer originates from a SingletonFactory whose
        // singleton lives in a 'static OnceLock. It is valid for the entire
        // program lifetime and properly aligned for T.
        #[allow(unsafe_code)]
        opt_ptr.map(|ptr| unsafe { &*(ptr as *const T) })
    }

    /// Resolves the interface by looking up the TfType by name, then calling
    /// the registered `ErasedPlugFactory`.
    ///
    /// Returns `Some(ptr)` on success, `None` on any failure.
    ///
    /// Mirrors C++ `Plug_StaticInterfaceBase::_LoadAndInstantiate`.
    fn load_and_instantiate(&self) -> Option<*const ()> {
        let tf_type = usd_tf::TfType::find_by_name(self.type_name);
        if tf_type.is_unknown() {
            log::warn!(
                "PlugStaticInterface: unknown type '{}'; plugin not registered",
                self.type_name
            );
            return None;
        }

        // Load the plugin that provides this type, if any.
        let registry = crate::registry::PlugRegistry::get_instance();
        if let Some(plugin) = registry.get_plugin_for_type(self.type_name) {
            if let Err(err) = plugin.load() {
                log::error!(
                    "PlugStaticInterface: failed to load plugin for '{}': {}",
                    self.type_name,
                    err
                );
                return None;
            }
        }
        // If there is no plugin entry the type may be statically linked;
        // proceed to factory lookup regardless.

        let factory_arc = tf_type.get_factory()?;
        let ptr = get_instance_from_factory(factory_arc.as_ref())?;

        if ptr.is_null() {
            log::error!(
                "PlugStaticInterface: factory returned null for type '{}'",
                self.type_name
            );
            return None;
        }

        Some(ptr)
    }
}

/// Calls the type-erased `new_instance` on a `dyn FactoryBase`.
///
/// Matches C++ `Plug_InterfaceFactory::Base::New()`. Works by downcasting the
/// `dyn FactoryBase` to `ErasedPlugFactory` (which wraps a `SingletonFactory`)
/// through `Any`.
fn get_instance_from_factory(factory: &dyn usd_tf::FactoryBase) -> Option<*const ()> {
    let erased = factory.as_any().downcast_ref::<ErasedPlugFactory>()?;
    Some((erased.new_fn)(erased.factory_ptr))
}

/// Type-erased shim that exposes `new_instance()` for any `SingletonFactory<T>`.
///
/// Because `PlugInterfaceFactory` (a sub-trait of `FactoryBase`) cannot be
/// directly recovered from a `dyn FactoryBase` via safe Rust, we store a plain
/// fn pointer alongside the factory data pointer. This gives us the same
/// single-virtual-call dispatch as C++ `Plug_InterfaceFactory::Base::New()`.
///
/// Register via `ErasedPlugFactory::from_singleton_factory`.
pub struct ErasedPlugFactory {
    /// Raw pointer to the `SingletonFactory<T>` data (kept alive by the Arc).
    factory_ptr: *const (),
    /// Thunk that calls `SingletonFactory<T>::new_instance(factory_ptr)`.
    new_fn: fn(*const ()) -> *const (),
}

// SAFETY: factory_ptr points to Arc-owned data for a SingletonFactory<T: Send+Sync>.
#[allow(unsafe_code)]
unsafe impl Send for ErasedPlugFactory {}
#[allow(unsafe_code)]
unsafe impl Sync for ErasedPlugFactory {}

impl ErasedPlugFactory {
    /// Constructs an `ErasedPlugFactory` from a reference to a
    /// `SingletonFactory<T>`.
    ///
    /// The caller must ensure the factory's `Arc` outlives this wrapper.
    /// This is always satisfied when both are stored inside the same `Arc`.
    pub fn from_singleton_factory<T: Send + Sync + 'static>(
        factory: &crate::interface_factory::SingletonFactory<T>,
    ) -> Self {
        fn call_new<U: Send + Sync + 'static>(ptr: *const ()) -> *const () {
            use crate::interface_factory::PlugInterfaceFactory;
            // SAFETY: ptr was cast from &SingletonFactory<U> which is valid
            // for the duration of the Arc that owns it.
            #[allow(unsafe_code)]
            let factory =
                unsafe { &*(ptr as *const crate::interface_factory::SingletonFactory<U>) };
            factory.new_instance()
        }
        Self {
            factory_ptr: factory as *const _ as *const (),
            new_fn: call_new::<T>,
        }
    }
}

impl usd_tf::FactoryBase for ErasedPlugFactory {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    struct HelloImpl {
        value: i32,
    }

    #[test]
    fn test_not_initialized_before_get() {
        let iface: PlugStaticInterface<HelloImpl> =
            PlugStaticInterface::new("NonExistentType::ForTest");
        assert!(!iface.is_initialized());
    }

    #[test]
    fn test_get_returns_none_for_unknown_type() {
        let iface: PlugStaticInterface<HelloImpl> =
            PlugStaticInterface::new("__no_such_type_xyz__");
        let result = iface.get();
        assert!(iface.is_initialized());
        assert!(result.is_none());
    }

    #[test]
    fn test_erased_factory_new_fn() {
        use crate::interface_factory::SingletonFactory;

        let factory = SingletonFactory::new(|| HelloImpl { value: 42 });
        let erased = ErasedPlugFactory::from_singleton_factory(&factory);

        let ptr = (erased.new_fn)(erased.factory_ptr) as *const HelloImpl;
        // SAFETY: ptr points into the SingletonFactory's OnceLock; valid for
        // the lifetime of `factory` on this stack frame.
        #[allow(unsafe_code)]
        let val = unsafe { (*ptr).value };
        assert_eq!(val, 42);

        // Second call must return the same pointer (singleton semantics).
        let ptr2 = (erased.new_fn)(erased.factory_ptr) as *const HelloImpl;
        assert_eq!(ptr, ptr2);
    }

    #[test]
    fn test_erased_factory_roundtrip_via_factory_base() {
        use crate::interface_factory::SingletonFactory;

        let factory = SingletonFactory::new(|| HelloImpl { value: 7 });
        let erased = ErasedPlugFactory::from_singleton_factory(&factory);

        // Simulate what TfType stores: Arc<dyn FactoryBase>.
        let factory_base: Arc<dyn usd_tf::FactoryBase> = Arc::new(erased);

        let ptr = get_instance_from_factory(factory_base.as_ref());
        assert!(ptr.is_some());
        // SAFETY: see above.
        #[allow(unsafe_code)]
        let val = unsafe { &*(ptr.unwrap() as *const HelloImpl) }.value;
        assert_eq!(val, 7);
    }
}
