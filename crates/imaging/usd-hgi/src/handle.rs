//! Handle wrapper for GPU resources with unique ID tracking

use std::sync::Arc;

/// Handle that contains an HGI object and unique id
///
/// The unique id is used to compare two handles to guard against pointer
/// aliasing, where the same memory address is used to create a similar object,
/// but it is not actually the same object.
///
/// Unlike the C++ version which uses raw pointers, this Rust version uses Arc
/// for safe shared ownership. The resource lifetime is still managed explicitly
/// through the Hgi Destroy*** functions.
pub struct HgiHandle<T: ?Sized> {
    ptr: Option<Arc<T>>,
    id: u64,
}

impl<T: ?Sized> HgiHandle<T> {
    /// Create a new handle with an object and unique ID
    pub fn new(obj: Arc<T>, id: u64) -> Self {
        Self { ptr: Some(obj), id }
    }

    /// Create an empty (null) handle
    pub fn null() -> Self {
        Self { ptr: None, id: 0 }
    }

    /// Create a handle with only an ID (no object).
    /// Used as a placeholder when the actual GPU resource is managed externally.
    pub fn with_id(id: u64) -> Self {
        Self { ptr: None, id }
    }

    /// Get a reference to the contained object
    pub fn get(&self) -> Option<&T> {
        self.ptr.as_ref().map(|arc| arc.as_ref())
    }

    /// Get the unique ID
    pub fn id(&self) -> u64 {
        self.id
    }

    /// Check if this handle contains a valid object
    pub fn is_valid(&self) -> bool {
        self.ptr.is_some()
    }

    /// Check if this handle is null/empty
    pub fn is_null(&self) -> bool {
        self.ptr.is_none()
    }

    /// Get a cloned Arc to the inner object
    pub fn arc(&self) -> Option<Arc<T>> {
        self.ptr.clone()
    }
}

impl<T: ?Sized> Clone for HgiHandle<T> {
    fn clone(&self) -> Self {
        Self {
            ptr: self.ptr.clone(),
            id: self.id,
        }
    }
}

impl<T: ?Sized> Default for HgiHandle<T> {
    fn default() -> Self {
        Self::null()
    }
}

impl<T: ?Sized> PartialEq for HgiHandle<T> {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl<T: ?Sized> Eq for HgiHandle<T> {}

impl<T: ?Sized> std::hash::Hash for HgiHandle<T> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.id.hash(state);
    }
}

impl<T: ?Sized> std::fmt::Debug for HgiHandle<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HgiHandle")
            .field("id", &self.id)
            .field("is_valid", &self.is_valid())
            .finish()
    }
}

// Convenience: allow dereferencing when valid
impl<T: ?Sized> std::ops::Deref for HgiHandle<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.get().expect("Attempted to dereference null HgiHandle")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestResource {
        value: i32,
    }

    #[test]
    fn test_handle_creation() {
        let resource = Arc::new(TestResource { value: 42 });
        let handle = HgiHandle::new(resource, 1);

        assert!(handle.is_valid());
        assert!(!handle.is_null());
        assert_eq!(handle.id(), 1);
        assert_eq!(handle.get().unwrap().value, 42);
    }

    #[test]
    fn test_null_handle() {
        let handle: HgiHandle<TestResource> = HgiHandle::null();

        assert!(!handle.is_valid());
        assert!(handle.is_null());
        assert_eq!(handle.id(), 0);
        assert!(handle.get().is_none());
    }

    #[test]
    fn test_handle_equality() {
        let resource1 = Arc::new(TestResource { value: 1 });
        let resource2 = Arc::new(TestResource { value: 2 });

        let handle1 = HgiHandle::new(resource1.clone(), 1);
        let handle2 = HgiHandle::new(resource1.clone(), 1);
        let handle3 = HgiHandle::new(resource2, 2);

        assert_eq!(handle1, handle2); // Same ID
        assert_ne!(handle1, handle3); // Different ID
    }

    #[test]
    fn test_handle_clone() {
        let resource = Arc::new(TestResource { value: 99 });
        let handle1 = HgiHandle::new(resource, 1);
        let handle2 = handle1.clone();

        assert_eq!(handle1, handle2);
        assert_eq!(handle1.id(), handle2.id());
        assert_eq!(handle1.get().unwrap().value, handle2.get().unwrap().value);
    }

    #[test]
    fn test_handle_deref() {
        let resource = Arc::new(TestResource { value: 123 });
        let handle = HgiHandle::new(resource, 1);

        // Test deref
        assert_eq!(handle.value, 123);
    }

    #[test]
    #[should_panic(expected = "Attempted to dereference null HgiHandle")]
    fn test_null_deref_panics() {
        let handle: HgiHandle<TestResource> = HgiHandle::null();
        let _ = handle.value; // Should panic
    }

    #[test]
    fn test_default() {
        let handle: HgiHandle<TestResource> = HgiHandle::default();
        assert!(handle.is_null());
    }
}
