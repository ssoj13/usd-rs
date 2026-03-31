//! Trace event categories.
//!
//! Categories allow trace events to be filtered by type. Each category has
//! a unique 32-bit identifier and an optional human-readable name.
//!
//! # Examples
//!
//! ```
//! use usd_trace::{Category, CategoryId, create_category_id};
//!
//! // Create category IDs from string literals
//! const MY_CATEGORY: CategoryId = create_category_id("MyCategory");
//!
//! // Register with the singleton
//! Category::get().register_category(MY_CATEGORY, "MyCategory");
//!
//! // Get names for a category
//! let names = Category::get().get_categories(MY_CATEGORY);
//! assert!(names.contains(&"MyCategory".to_string()));
//! ```

use std::collections::{HashMap, HashSet};
use std::sync::{LazyLock, RwLock};

/// Category identifier for trace events.
pub type CategoryId = u32;

/// Default category ID (0) used when no category is explicitly specified.
pub const DEFAULT_CATEGORY: CategoryId = 0;

/// Computes a category ID from a string at compile time using FNV-1a hash.
///
/// # Examples
///
/// ```
/// use usd_trace::{create_category_id, CategoryId};
///
/// const MY_ID: CategoryId = create_category_id("MyCategory");
/// ```
#[must_use]
pub const fn create_category_id(s: &str) -> CategoryId {
    // FNV-1a hash for 32-bit
    const FNV_PRIME: u32 = 16777619;
    const FNV_OFFSET: u32 = 2166136261;

    let bytes = s.as_bytes();
    let mut hash = FNV_OFFSET;
    let mut i = 0;
    while i < bytes.len() {
        hash ^= bytes[i] as u32;
        hash = hash.wrapping_mul(FNV_PRIME);
        i += 1;
    }
    hash
}

/// Global category registry singleton.
static CATEGORY: LazyLock<Category> = LazyLock::new(Category::new);

/// Singleton for managing trace event categories.
///
/// Categories allow trace events to be grouped and filtered by type.
/// Each category is identified by a [`CategoryId`] and may have one or
/// more human-readable names associated with it.
///
/// # Examples
///
/// ```
/// use usd_trace::{Category, create_category_id};
///
/// let category = Category::get();
///
/// // Register a category
/// const RENDER: u32 = create_category_id("Render");
/// category.register_category(RENDER, "Render");
///
/// // Get category names
/// let names = category.get_categories(RENDER);
/// assert!(names.contains(&"Render".to_string()));
/// ```
pub struct Category {
    /// Mapping of category IDs to names (multiple names per ID allowed).
    id_to_names: RwLock<HashMap<CategoryId, Vec<String>>>,
    /// Set of disabled categories. If empty, all categories are enabled.
    disabled: RwLock<HashSet<CategoryId>>,
}

impl Category {
    /// Creates a new category registry.
    fn new() -> Self {
        Self {
            id_to_names: RwLock::new(HashMap::new()),
            disabled: RwLock::new(HashSet::new()),
        }
    }

    /// Returns the singleton instance.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_trace::Category;
    ///
    /// let category = Category::get();
    /// ```
    #[must_use]
    pub fn get() -> &'static Category {
        &CATEGORY
    }

    /// Associates a category ID with a human-readable name.
    ///
    /// Multiple names can be associated with the same ID.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_trace::{Category, create_category_id};
    ///
    /// const MY_CAT: u32 = create_category_id("MyCat");
    /// Category::get().register_category(MY_CAT, "MyCat");
    /// Category::get().register_category(MY_CAT, "MyCategory"); // Alternative name
    /// ```
    pub fn register_category(&self, id: CategoryId, name: &str) {
        if let Ok(mut map) = self.id_to_names.write() {
            map.entry(id).or_default().push(name.to_string());
        }
    }

    /// Returns all names associated with a category ID.
    ///
    /// Returns an empty vector if no names are registered for the ID.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_trace::{Category, create_category_id, DEFAULT_CATEGORY};
    ///
    /// // Unregistered category returns empty
    /// let names = Category::get().get_categories(DEFAULT_CATEGORY);
    /// // May or may not be empty depending on registration
    /// ```
    #[must_use]
    pub fn get_categories(&self, id: CategoryId) -> Vec<String> {
        if let Ok(map) = self.id_to_names.read() {
            map.get(&id).cloned().unwrap_or_default()
        } else {
            Vec::new()
        }
    }

    /// Returns true if the category ID has any registered names.
    #[must_use]
    pub fn is_registered(&self, id: CategoryId) -> bool {
        if let Ok(map) = self.id_to_names.read() {
            map.contains_key(&id)
        } else {
            false
        }
    }

    /// Returns the number of registered category IDs.
    #[must_use]
    pub fn count(&self) -> usize {
        if let Ok(map) = self.id_to_names.read() {
            map.len()
        } else {
            0
        }
    }

    /// Returns true if the category is enabled (i.e. not in the disabled set).
    ///
    /// By default all categories are enabled. Use [`disable_category`](Self::disable_category)
    /// to suppress events for a specific category.
    #[must_use]
    #[inline]
    pub fn is_enabled(&self, id: CategoryId) -> bool {
        if let Ok(set) = self.disabled.read() {
            !set.contains(&id)
        } else {
            true
        }
    }

    /// Disables a category so events tagged with it are suppressed.
    pub fn disable_category(&self, id: CategoryId) {
        if let Ok(mut set) = self.disabled.write() {
            set.insert(id);
        }
    }

    /// Re-enables a previously disabled category.
    pub fn enable_category(&self, id: CategoryId) {
        if let Ok(mut set) = self.disabled.write() {
            set.remove(&id);
        }
    }

    /// Re-enables all categories (clears the disabled set).
    pub fn enable_all(&self) {
        if let Ok(mut set) = self.disabled.write() {
            set.clear();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    /// Serializes tests that mutate the global Category singleton to avoid races.
    static CATEGORY_TEST_LOCK: LazyLock<Mutex<()>> = LazyLock::new(Mutex::default);

    #[test]
    fn test_create_category_id() {
        const ID1: CategoryId = create_category_id("Test");
        const ID2: CategoryId = create_category_id("Test");
        const ID3: CategoryId = create_category_id("Other");

        // Same string should produce same ID
        assert_eq!(ID1, ID2);

        // Different strings should (usually) produce different IDs
        assert_ne!(ID1, ID3);
    }

    #[test]
    fn test_default_category() {
        assert_eq!(DEFAULT_CATEGORY, 0);
    }

    #[test]
    fn test_get_singleton() {
        let _guard = CATEGORY_TEST_LOCK.lock().unwrap();
        let cat1 = Category::get();
        let cat2 = Category::get();

        // Should be the same instance
        assert!(std::ptr::eq(cat1, cat2));
    }

    #[test]
    fn test_register_and_get() {
        let _guard = CATEGORY_TEST_LOCK.lock().unwrap();
        let category = Category::get();

        const TEST_CAT: CategoryId = create_category_id("TestCat_RegisterGet");
        category.register_category(TEST_CAT, "TestCat");

        let names = category.get_categories(TEST_CAT);
        assert!(names.contains(&"TestCat".to_string()));
    }

    #[test]
    fn test_multiple_names() {
        let _guard = CATEGORY_TEST_LOCK.lock().unwrap();
        let category = Category::get();

        const MULTI_CAT: CategoryId = create_category_id("MultiNameCat");
        category.register_category(MULTI_CAT, "Name1");
        category.register_category(MULTI_CAT, "Name2");

        let names = category.get_categories(MULTI_CAT);
        assert!(names.contains(&"Name1".to_string()));
        assert!(names.contains(&"Name2".to_string()));
    }

    #[test]
    fn test_unregistered_category() {
        let _guard = CATEGORY_TEST_LOCK.lock().unwrap();
        let category = Category::get();

        const UNKNOWN: CategoryId = create_category_id("NeverRegistered12345");
        let names = category.get_categories(UNKNOWN);

        // Unregistered category may return empty (unless registered elsewhere in tests)
        // Just verify we can call it without panic
        let _ = names;
    }

    #[test]
    fn test_is_registered() {
        let _guard = CATEGORY_TEST_LOCK.lock().unwrap();
        let category = Category::get();

        const REG_CAT: CategoryId = create_category_id("RegisteredCategory");

        category.register_category(REG_CAT, "Registered");

        assert!(category.is_registered(REG_CAT));
    }

    #[test]
    fn test_category_enable_disable() {
        let _guard = CATEGORY_TEST_LOCK.lock().unwrap();
        let cat = Category::get();
        const FILTER_CAT: CategoryId = create_category_id("FilterTest");

        // All categories enabled by default
        assert!(cat.is_enabled(FILTER_CAT));

        // Disable
        cat.disable_category(FILTER_CAT);
        assert!(!cat.is_enabled(FILTER_CAT));

        // Re-enable
        cat.enable_category(FILTER_CAT);
        assert!(cat.is_enabled(FILTER_CAT));
    }

    #[test]
    fn test_category_enable_all() {
        let _guard = CATEGORY_TEST_LOCK.lock().unwrap();
        let cat = Category::get();
        const A: CategoryId = create_category_id("EnableAllA");
        const B: CategoryId = create_category_id("EnableAllB");

        cat.disable_category(A);
        cat.disable_category(B);
        assert!(!cat.is_enabled(A));
        assert!(!cat.is_enabled(B));

        cat.enable_all();
        assert!(cat.is_enabled(A));
        assert!(cat.is_enabled(B));
    }

    #[test]
    fn test_default_category_always_enabled() {
        let _guard = CATEGORY_TEST_LOCK.lock().unwrap();
        let cat = Category::get();
        // Default category (0) should be enabled unless explicitly disabled
        assert!(cat.is_enabled(DEFAULT_CATEGORY));
    }
}
