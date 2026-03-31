//! Common value utilities.
//!
//! Provides utilities for default value creation and type storage mapping.
//! This module matches the functionality of `pxr/base/vt/valueCommon.h`.

use std::any::TypeId;
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

/// Helper class used by DefaultValueFactory to return a value with
/// its type erased and only known at runtime via TypeId.
///
/// Matches C++ `Vt_DefaultValueHolder`.
pub struct DefaultValueHolder {
    /// The held value (type-erased, boxed for stable pointer).
    /// We use Box<dyn Any> instead of AnyUniquePtr to get stable pointers.
    ptr: Box<dyn std::any::Any + Send + Sync>,
    /// The TypeId of the held type.
    type_id: TypeId,
}

impl DefaultValueHolder {
    /// Creates a value-initialized object and stores the TypeId for the static type.
    ///
    /// Matches C++ `Vt_DefaultValueHolder::Create<T>()`.
    pub fn create<T: Default + Send + Sync + 'static>() -> Self {
        Self {
            ptr: Box::new(T::default()),
            type_id: TypeId::of::<T>(),
        }
    }

    /// Creates a copy of the object and stores the TypeId for the static type.
    ///
    /// Matches C++ `Vt_DefaultValueHolder::Create<T>(val)`.
    pub fn create_with_value<T: Clone + Send + Sync + 'static>(val: &T) -> Self {
        Self {
            ptr: Box::new(val.clone()),
            type_id: TypeId::of::<T>(),
        }
    }

    /// Return the runtime type of the held object.
    ///
    /// Matches C++ `Vt_DefaultValueHolder::GetType()`.
    pub fn get_type(&self) -> TypeId {
        self.type_id
    }

    /// Return a pointer to the held object.
    ///
    /// This may be safely cast to the static type corresponding to the TypeId
    /// returned by `get_type()`.
    ///
    /// Matches C++ `Vt_DefaultValueHolder::GetPointer()`.
    ///
    /// # Safety
    ///
    /// The caller must ensure that T matches the type_id returned by `get_type()`.
    #[allow(unsafe_code)]
    pub unsafe fn get_pointer<T: 'static>(&self) -> *const T {
        if self.type_id == TypeId::of::<T>() {
            // SAFETY: TypeId check ensures type correctness
            #[allow(unsafe_code)]
            {
                self.ptr
                    .downcast_ref::<T>()
                    .map(|r| r as *const T)
                    .unwrap_or(std::ptr::null())
            }
        } else {
            std::ptr::null()
        }
    }

    /// Return a raw pointer to the held object.
    ///
    /// This is used internally to get a stable pointer that can be stored.
    /// The pointer remains valid as long as this holder exists.
    ///
    /// SAFETY INVARIANT: We avoid transmute by using a helper function that works
    /// with the concrete type. This is called via get_pointer_raw_impl which does
    /// the actual unsafe cast based on stored type_id.
    ///
    /// # Safety
    ///
    /// The returned pointer must be cast to the type corresponding to `type_id`.
    /// This is a low-level API for C++ compatibility - prefer typed get_pointer<T>().
    #[allow(unsafe_code)]
    pub unsafe fn get_pointer_raw(&self) -> *const u8 {
        // SAFETY: We get the trait object reference and convert it to a raw pointer.
        // The Box guarantees the value won't move, so the pointer remains stable.
        // We use as_ref() on the Box to avoid any transmute operations.
        let any_ref: &dyn std::any::Any = self.ptr.as_ref();
        // Get raw pointer to the trait object's data by using the fat pointer directly
        // Cast to *const u8 for type erasure
        any_ref as *const dyn std::any::Any as *const u8
    }
}

/// Trait for creating default values.
///
/// VtValue uses this to create values to be returned from failed calls to Get.
/// Clients may specialize this for their own types.
///
/// Matches C++ `Vt_DefaultValueFactory` template struct.
pub trait DefaultValueFactory<T: 'static> {
    /// Creates a default value holder for type T.
    ///
    /// Matches C++ `Vt_DefaultValueFactory<T>::Invoke()`.
    fn invoke() -> DefaultValueHolder;
}

/// Default implementation: creates a value-initialized T.
///
/// Matches C++ default implementation of `Vt_DefaultValueFactory<T>::Invoke()`.
impl<T: Default + Send + Sync + 'static> DefaultValueFactory<T> for T {
    fn invoke() -> DefaultValueHolder {
        DefaultValueHolder::create::<T>()
    }
}

/// Returns a default value for the given type.
///
/// This function stores a global map from type name to value. If we have an entry
/// for the requested type in the map already, return that. Otherwise use the factory
/// function to create a new entry to store in the map.
///
/// Matches C++ `Vt_FindOrCreateDefaultValue`.
///
/// SAFETY INVARIANT: All unsafe operations are centralized in get_pointer_raw() calls.
/// Type safety is enforced by storing TypeId with each holder and verifying it matches.
///
/// # Arguments
///
/// * `type_id` - The TypeId of the type to get a default value for
/// * `factory` - Function that creates a DefaultValueHolder for this type
///
/// # Returns
///
/// A raw pointer to the default value. The caller must cast this to the correct type.
/// The pointer remains valid for the lifetime of the program.
///
/// # Safety
///
/// The returned pointer must be cast to the type corresponding to `type_id`.
/// The pointer is valid as long as the static map exists (lifetime of the program).
#[allow(unsafe_code)]
pub unsafe fn find_or_create_default_value<F>(type_id: TypeId, factory: F) -> *const u8
where
    F: FnOnce() -> DefaultValueHolder,
{
    // Static map from type name to default value holder
    // Matches C++: static DefaultValuesMap defaultValues;
    static DEFAULT_VALUES: OnceLock<Mutex<HashMap<String, Box<DefaultValueHolder>>>> =
        OnceLock::new();

    let map = DEFAULT_VALUES.get_or_init(|| Mutex::new(HashMap::new()));

    // Get type name for key (matches C++ ArchGetDemangled(type))
    // Use TypeId as key since we can't easily get demangled name from TypeId
    let type_name = format!("{:?}", type_id);

    // Try to find existing entry (with lock held)
    {
        let guard = map.lock().expect("lock poisoned");
        if let Some(holder) = guard.get(&type_name) {
            // Verify type matches
            if holder.get_type() == type_id {
                // SAFETY: TypeId verified, pointer from static map (lifetime: 'static)
                #[allow(unsafe_code)]
                return unsafe { holder.get_pointer_raw() };
            }
        }
    }

    // Create new entry (factory called outside lock to avoid deadlock)
    // Matches C++: Vt_DefaultValueHolder newValue = factory();
    let new_value = factory();

    // Verify type matches
    // Matches C++: TF_AXIOM(TfSafeTypeCompare(newValue.GetType(), type));
    assert_eq!(new_value.get_type(), type_id, "Factory produced wrong type");

    // Insert into map (with lock held)
    // Matches C++: defaultValues.emplace(std::move(key), std::move(newValue))
    let mut guard = map.lock().expect("lock poisoned");

    // Double-check after acquiring lock (another thread might have inserted)
    // Matches C++ double-check pattern
    if let Some(holder) = guard.get(&type_name) {
        if holder.get_type() == type_id {
            // SAFETY: TypeId verified, pointer from static map (lifetime: 'static)
            #[allow(unsafe_code)]
            return unsafe { holder.get_pointer_raw() };
        }
    }

    // Insert into map (holder is now owned by the map)
    // Get pointer before inserting (we need to clone type_name for lookup after insert)
    let type_name_clone = type_name.clone();
    guard.insert(type_name, Box::new(new_value));

    // Get pointer to the value we just inserted
    // The pointer remains valid because the holder is stored in the static map
    if let Some(holder) = guard.get(&type_name_clone) {
        // SAFETY: We just inserted this with verified type_id, pointer from static map
        #[allow(unsafe_code)]
        unsafe {
            holder.get_pointer_raw()
        }
    } else {
        std::ptr::null()
    }
}

/// Metafunction that gives the type Value should store for a given type T.
///
/// By default, stores T as-is. Specializations can map types (e.g., char* -> String).
///
/// Matches C++ `Vt_ValueStoredType` template struct.
///
/// Note: In Rust, we don't need specializations for char* since we use String/&str directly.
/// The C++ specializations for char* -> std::string are handled by Rust's type system.
pub trait ValueStoredType {
    /// The type that should be stored for Self.
    type Type;
}

/// Default: store T as-is.
///
/// This covers all types except those that need special handling.
/// In C++, char* is specialized to std::string, but in Rust we use String/&str directly.
impl<T> ValueStoredType for T {
    type Type = T;
}

/// Metafunction that gives the type Value should store for a given type T.
///
/// Uses std::decay_t equivalent (removes references, const, volatile).
///
/// Matches C++ `Vt_ValueGetStored` template alias.
///
/// In Rust, we typically store owned types (T) rather than references (&T),
/// so this is mainly for API compatibility with C++.
///
/// # Example
///
/// ```
/// use usd_vt::value_common::ValueGetStored;
///
/// // ValueGetStored<i32> = i32
/// // ValueGetStored<String> = String
/// ```
pub type ValueGetStored<T> = <T as ValueStoredType>::Type;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_value_holder_create() {
        let holder = DefaultValueHolder::create::<i32>();
        assert_eq!(holder.get_type(), TypeId::of::<i32>());
    }

    #[test]
    fn test_default_value_holder_create_with_value() {
        let holder = DefaultValueHolder::create_with_value(&42i32);
        assert_eq!(holder.get_type(), TypeId::of::<i32>());
        // SAFETY: Test verifies type matches via get_type() check
        #[allow(unsafe_code)]
        unsafe {
            let ptr = holder.get_pointer::<i32>();
            assert!(!ptr.is_null());
            assert_eq!(*ptr, 42);
        }
    }

    #[test]
    fn test_default_value_factory() {
        let holder = <i32 as DefaultValueFactory<i32>>::invoke();
        assert_eq!(holder.get_type(), TypeId::of::<i32>());
    }
}
