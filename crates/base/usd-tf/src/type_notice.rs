//! Type declaration notices.
//!
//! Provides notices that are sent when types are declared in the type system.
//!
//! # Overview
//!
//! [`TypeWasDeclaredNotice`] is sent after a type is registered with the
//! type system, allowing listeners to react to new type registrations.
//!
//! # Examples
//!
//! ```
//! use usd_tf::type_notice::TypeWasDeclaredNotice;
//! use std::any::TypeId;
//!
//! // Create a notice for a type declaration
//! let notice = TypeWasDeclaredNotice::new::<String>();
//! assert_eq!(notice.type_id(), TypeId::of::<String>());
//! ```

use std::any::TypeId;

/// Notice sent after a type is declared.
///
/// This notice is broadcast when a new type is registered with the type system,
/// allowing interested parties to react to type declarations (e.g., for
/// plugin loading, schema registration, etc.).
///
/// # Examples
///
/// ```
/// use usd_tf::type_notice::TypeWasDeclaredNotice;
/// use std::any::TypeId;
///
/// let notice = TypeWasDeclaredNotice::new::<i32>();
/// assert_eq!(notice.type_id(), TypeId::of::<i32>());
/// assert_eq!(notice.type_name(), "i32");
/// ```
#[derive(Debug, Clone)]
pub struct TypeWasDeclaredNotice {
    /// The TypeId of the declared type.
    type_id: TypeId,
    /// The name of the declared type.
    type_name: &'static str,
}

impl TypeWasDeclaredNotice {
    /// Create a new notice for a type declaration.
    ///
    /// # Type Parameters
    ///
    /// - `T`: The type that was declared
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_tf::type_notice::TypeWasDeclaredNotice;
    ///
    /// let notice = TypeWasDeclaredNotice::new::<Vec<u8>>();
    /// assert!(notice.type_name().contains("Vec"));
    /// ```
    pub fn new<T: 'static>() -> Self {
        Self {
            type_id: TypeId::of::<T>(),
            type_name: std::any::type_name::<T>(),
        }
    }

    /// Create a new notice with explicit type ID and name.
    ///
    /// This is useful when the type information is available at runtime
    /// rather than compile time.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_tf::type_notice::TypeWasDeclaredNotice;
    /// use std::any::TypeId;
    ///
    /// let notice = TypeWasDeclaredNotice::with_info(
    ///     TypeId::of::<String>(),
    ///     "String"
    /// );
    /// assert_eq!(notice.type_name(), "String");
    /// ```
    pub fn with_info(type_id: TypeId, type_name: &'static str) -> Self {
        Self { type_id, type_name }
    }

    /// Get the [`TypeId`] of the declared type.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_tf::type_notice::TypeWasDeclaredNotice;
    /// use std::any::TypeId;
    ///
    /// let notice = TypeWasDeclaredNotice::new::<bool>();
    /// assert_eq!(notice.type_id(), TypeId::of::<bool>());
    /// ```
    #[inline]
    pub fn type_id(&self) -> TypeId {
        self.type_id
    }

    /// Get the name of the declared type.
    ///
    /// This returns the result of `std::any::type_name::<T>()` for the type.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_tf::type_notice::TypeWasDeclaredNotice;
    ///
    /// let notice = TypeWasDeclaredNotice::new::<String>();
    /// assert!(notice.type_name().contains("String"));
    /// ```
    #[inline]
    pub fn type_name(&self) -> &'static str {
        self.type_name
    }

    /// Check if this notice is for a specific type.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_tf::type_notice::TypeWasDeclaredNotice;
    ///
    /// let notice = TypeWasDeclaredNotice::new::<i32>();
    /// assert!(notice.is_type::<i32>());
    /// assert!(!notice.is_type::<i64>());
    /// ```
    #[inline]
    pub fn is_type<T: 'static>(&self) -> bool {
        self.type_id == TypeId::of::<T>()
    }
}

impl PartialEq for TypeWasDeclaredNotice {
    fn eq(&self, other: &Self) -> bool {
        self.type_id == other.type_id
    }
}

impl Eq for TypeWasDeclaredNotice {}

impl std::hash::Hash for TypeWasDeclaredNotice {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.type_id.hash(state);
    }
}

/// A listener for type declaration notices.
///
/// This trait can be implemented to receive notifications when types are declared.
pub trait TypeWasDeclaredListener {
    /// Called when a type is declared.
    ///
    /// # Parameters
    ///
    /// - `notice`: The notice containing information about the declared type
    fn on_type_declared(&mut self, notice: &TypeWasDeclaredNotice);
}

/// Type alias for notice listener callback.
type NoticeListener = Box<dyn FnMut(&TypeWasDeclaredNotice)>;

/// Registry for type declaration listeners.
///
/// This provides a simple mechanism to register and notify listeners
/// when types are declared.
///
/// # Examples
///
/// ```
/// use usd_tf::type_notice::{TypeWasDeclaredNotice, TypeNoticeRegistry};
///
/// let mut registry = TypeNoticeRegistry::new();
///
/// // Register a callback
/// registry.register(|notice| {
///     println!("Type declared: {}", notice.type_name());
/// });
///
/// // Send a notice
/// let notice = TypeWasDeclaredNotice::new::<String>();
/// registry.send(&notice);
/// ```
pub struct TypeNoticeRegistry {
    listeners: Vec<NoticeListener>,
}

impl TypeNoticeRegistry {
    /// Create a new empty registry.
    pub fn new() -> Self {
        Self {
            listeners: Vec::new(),
        }
    }

    /// Register a callback to be called when types are declared.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_tf::type_notice::{TypeWasDeclaredNotice, TypeNoticeRegistry};
    ///
    /// let mut registry = TypeNoticeRegistry::new();
    /// let mut count = 0;
    ///
    /// registry.register(move |_notice| {
    ///     // Process notice
    /// });
    /// ```
    pub fn register<F>(&mut self, callback: F)
    where
        F: FnMut(&TypeWasDeclaredNotice) + 'static,
    {
        self.listeners.push(Box::new(callback));
    }

    /// Send a notice to all registered listeners.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_tf::type_notice::{TypeWasDeclaredNotice, TypeNoticeRegistry};
    ///
    /// let mut registry = TypeNoticeRegistry::new();
    /// let notice = TypeWasDeclaredNotice::new::<i32>();
    /// registry.send(&notice);
    /// ```
    pub fn send(&mut self, notice: &TypeWasDeclaredNotice) {
        for listener in &mut self.listeners {
            listener(notice);
        }
    }

    /// Get the number of registered listeners.
    #[inline]
    pub fn listener_count(&self) -> usize {
        self.listeners.len()
    }

    /// Clear all registered listeners.
    pub fn clear(&mut self) {
        self.listeners.clear();
    }
}

impl Default for TypeNoticeRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for TypeNoticeRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TypeNoticeRegistry")
            .field("listener_count", &self.listeners.len())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::rc::Rc;

    #[test]
    fn test_notice_new() {
        let notice = TypeWasDeclaredNotice::new::<i32>();
        assert_eq!(notice.type_id(), TypeId::of::<i32>());
        assert_eq!(notice.type_name(), "i32");
    }

    #[test]
    fn test_notice_with_info() {
        let notice = TypeWasDeclaredNotice::with_info(TypeId::of::<String>(), "CustomString");
        assert_eq!(notice.type_id(), TypeId::of::<String>());
        assert_eq!(notice.type_name(), "CustomString");
    }

    #[test]
    fn test_notice_is_type() {
        let notice = TypeWasDeclaredNotice::new::<Vec<u8>>();
        assert!(notice.is_type::<Vec<u8>>());
        assert!(!notice.is_type::<Vec<i8>>());
    }

    #[test]
    fn test_notice_equality() {
        let n1 = TypeWasDeclaredNotice::new::<i32>();
        let n2 = TypeWasDeclaredNotice::new::<i32>();
        let n3 = TypeWasDeclaredNotice::new::<i64>();

        assert_eq!(n1, n2);
        assert_ne!(n1, n3);
    }

    #[test]
    fn test_notice_hash() {
        use std::collections::HashSet;

        let n1 = TypeWasDeclaredNotice::new::<i32>();
        let n2 = TypeWasDeclaredNotice::new::<i32>();
        let n3 = TypeWasDeclaredNotice::new::<i64>();

        let mut set = HashSet::new();
        set.insert(n1.clone());

        assert!(set.contains(&n2));
        assert!(!set.contains(&n3));
    }

    #[test]
    fn test_notice_clone() {
        let n1 = TypeWasDeclaredNotice::new::<String>();
        let n2 = n1.clone();

        assert_eq!(n1, n2);
        assert_eq!(n1.type_name(), n2.type_name());
    }

    #[test]
    fn test_notice_debug() {
        let notice = TypeWasDeclaredNotice::new::<i32>();
        let debug_str = format!("{:?}", notice);
        assert!(debug_str.contains("TypeWasDeclaredNotice"));
    }

    #[test]
    fn test_registry_new() {
        let registry = TypeNoticeRegistry::new();
        assert_eq!(registry.listener_count(), 0);
    }

    #[test]
    fn test_registry_register_and_send() {
        let mut registry = TypeNoticeRegistry::new();
        let count = Rc::new(RefCell::new(0));
        let count_clone = count.clone();

        registry.register(move |_notice| {
            *count_clone.borrow_mut() += 1;
        });

        assert_eq!(registry.listener_count(), 1);

        let notice = TypeWasDeclaredNotice::new::<i32>();
        registry.send(&notice);

        assert_eq!(*count.borrow(), 1);

        registry.send(&notice);
        assert_eq!(*count.borrow(), 2);
    }

    #[test]
    fn test_registry_multiple_listeners() {
        let mut registry = TypeNoticeRegistry::new();
        let count1 = Rc::new(RefCell::new(0));
        let count2 = Rc::new(RefCell::new(0));

        let c1 = count1.clone();
        let c2 = count2.clone();

        registry.register(move |_| {
            *c1.borrow_mut() += 1;
        });
        registry.register(move |_| {
            *c2.borrow_mut() += 10;
        });

        assert_eq!(registry.listener_count(), 2);

        let notice = TypeWasDeclaredNotice::new::<bool>();
        registry.send(&notice);

        assert_eq!(*count1.borrow(), 1);
        assert_eq!(*count2.borrow(), 10);
    }

    #[test]
    fn test_registry_clear() {
        let mut registry = TypeNoticeRegistry::new();

        registry.register(|_| {});
        registry.register(|_| {});

        assert_eq!(registry.listener_count(), 2);

        registry.clear();
        assert_eq!(registry.listener_count(), 0);
    }

    #[test]
    fn test_registry_default() {
        let registry = TypeNoticeRegistry::default();
        assert_eq!(registry.listener_count(), 0);
    }

    #[test]
    fn test_registry_debug() {
        let mut registry = TypeNoticeRegistry::new();
        registry.register(|_| {});

        let debug_str = format!("{:?}", registry);
        assert!(debug_str.contains("TypeNoticeRegistry"));
        assert!(debug_str.contains("1"));
    }

    #[test]
    fn test_listener_receives_correct_notice() {
        let mut registry = TypeNoticeRegistry::new();
        let received_type_id = Rc::new(RefCell::new(None));
        let rtid = received_type_id.clone();

        registry.register(move |notice| {
            *rtid.borrow_mut() = Some(notice.type_id());
        });

        let notice = TypeWasDeclaredNotice::new::<String>();
        registry.send(&notice);

        assert_eq!(*received_type_id.borrow(), Some(TypeId::of::<String>()));
    }
}
