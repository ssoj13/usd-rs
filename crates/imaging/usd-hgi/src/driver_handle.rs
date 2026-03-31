
//! HgiDriverHandle - Hgi wrapper for HdDriver Value storage.
//!
//! Used to pass Hgi through the HdDriver system (HgiTokens->renderDriver).
//! Port of storing Hgi* in VtValue in C++ HdStRenderDelegate::SetDrivers.

use super::hgi::Hgi;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use parking_lot::RwLock;

/// Handle for passing Hgi through HdDriver.
///
/// Wraps `Arc<RwLock<dyn Hgi>>` so it can be stored in `Value` and extracted
/// by `HdStRenderDelegate::set_drivers`.
pub struct HgiDriverHandle(Arc<RwLock<dyn Hgi + Send>>);

impl HgiDriverHandle {
    /// Create a new handle from shared Hgi.
    pub fn new(hgi: Arc<RwLock<dyn Hgi + Send>>) -> Self {
        Self(hgi)
    }

    /// Get shared reference to the Hgi wrapper.
    pub fn get(&self) -> &Arc<RwLock<dyn Hgi + Send>> {
        &self.0
    }

    /// Execute a closure with write lock (avoids lifetime issues with guard).
    /// Execute a closure with write lock.
    pub fn with_write<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&mut (dyn Hgi + Send)) -> R,
    {
        let mut guard = self.0.write();
        f(&mut *guard)
    }

    /// Execute a closure with read lock.
    pub fn with_read<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&(dyn Hgi + Send)) -> R,
    {
        let guard = self.0.read();
        f(&*guard)
    }
}

impl Clone for HgiDriverHandle {
    fn clone(&self) -> Self {
        Self(Arc::clone(&self.0))
    }
}

impl fmt::Debug for HgiDriverHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HgiDriverHandle")
            .field("hgi", &"Arc<RwLock<dyn Hgi>>")
            .finish()
    }
}

impl PartialEq for HgiDriverHandle {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

impl Eq for HgiDriverHandle {}

impl Hash for HgiDriverHandle {
    fn hash<H: Hasher>(&self, state: &mut H) {
        Arc::as_ptr(&self.0).hash(state);
    }
}

// Value integration: HgiDriverHandle can be stored in vt::Value
// Impl lives here to avoid circular deps (imaging -> base)
impl From<HgiDriverHandle> for usd_vt::Value {
    fn from(handle: HgiDriverHandle) -> Self {
        usd_vt::Value::new(handle)
    }
}
