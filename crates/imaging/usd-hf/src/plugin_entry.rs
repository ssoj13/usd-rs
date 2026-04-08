//! Internal plugin entry management.
//!
//! Manages individual plugin instances with ref counting and lazy instantiation.

use super::plugin_base::HfPluginBase;
use super::plugin_desc::HfPluginDesc;
use std::any::TypeId;
use std::sync::Arc;
use std::sync::RwLock;
use usd_tf::Token;

/// Factory function type for creating plugin instances.
pub type PluginFactoryFn = Box<dyn Fn() -> Box<dyn HfPluginBase> + Send + Sync>;

/// Internal plugin entry for managing a single plugin.
///
/// Handles ref counting, lazy instantiation, and plugin metadata.
/// This is an internal implementation detail of HfPluginRegistry.
pub struct HfPluginEntry {
    type_id: TypeId,
    type_name: String,
    display_name: String,
    priority: i32,
    instance: Arc<RwLock<Option<Box<dyn HfPluginBase>>>>,
    ref_count: Arc<RwLock<usize>>,
    factory: Arc<PluginFactoryFn>,
}

impl HfPluginEntry {
    /// Creates a new plugin entry.
    pub fn new(
        type_id: TypeId,
        type_name: String,
        display_name: String,
        priority: i32,
        factory: PluginFactoryFn,
    ) -> Self {
        Self {
            type_id,
            type_name,
            display_name,
            priority,
            instance: Arc::new(RwLock::new(None)),
            ref_count: Arc::new(RwLock::new(0)),
            factory: Arc::new(factory),
        }
    }

    /// Returns the type ID of this plugin.
    pub fn type_id(&self) -> TypeId {
        self.type_id
    }

    /// Returns the type name of this plugin.
    pub fn type_name(&self) -> &str {
        &self.type_name
    }

    /// Returns the display name of this plugin.
    pub fn display_name(&self) -> &str {
        &self.display_name
    }

    /// Returns the priority of this plugin.
    pub fn priority(&self) -> i32 {
        self.priority
    }

    /// Returns the plugin ID (same as type name).
    pub fn id(&self) -> Token {
        Token::new(&self.type_name)
    }

    /// Fills in a plugin description structure.
    pub fn get_desc(&self) -> HfPluginDesc {
        HfPluginDesc::new(self.id(), self.display_name.clone(), self.priority)
    }

    /// Increments the reference count and instantiates if needed.
    pub fn inc_ref_count(&self) {
        let mut ref_count = self.ref_count.write().expect("lock poisoned");

        if *ref_count == 0 {
            // Create instance on first reference
            let mut instance = self.instance.write().expect("lock poisoned");
            if instance.is_none() {
                *instance = Some((self.factory)());
            }
        }

        *ref_count += 1;
    }

    /// Decrements the reference count and destroys instance if zero.
    pub fn dec_ref_count(&self) {
        let mut ref_count = self.ref_count.write().expect("lock poisoned");

        if *ref_count == 0 {
            log::warn!("Plugin ref count underflow for '{}'", self.type_name);
            return;
        }

        *ref_count -= 1;

        if *ref_count == 0 {
            // Destroy instance when no more references
            let mut instance = self.instance.write().expect("lock poisoned");
            *instance = None;
        }
    }

    /// Returns current ref count (for testing/debugging).
    pub fn ref_count(&self) -> usize {
        *self.ref_count.read().expect("lock poisoned")
    }

    /// Checks if the plugin instance is currently loaded.
    pub fn is_loaded(&self) -> bool {
        self.instance.read().expect("lock poisoned").is_some()
    }

    /// Gets a reference to the plugin instance, if loaded.
    ///
    /// Returns None if instance hasn't been created yet.
    pub fn instance(&self) -> Option<Arc<RwLock<Option<Box<dyn HfPluginBase>>>>> {
        if self.is_loaded() {
            Some(Arc::clone(&self.instance))
        } else {
            None
        }
    }
}

impl PartialEq for HfPluginEntry {
    fn eq(&self, other: &Self) -> bool {
        // Compare by type_name rather than TypeId: erased plugins share
        // TypeId::of::<()>() as a placeholder, so TypeId is not reliable.
        // type_name is always unique (registry rejects duplicates).
        self.type_name == other.type_name
    }
}

impl Eq for HfPluginEntry {}

impl PartialOrd for HfPluginEntry {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for HfPluginEntry {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        // Sort by priority ASCENDING (lower number = sorts first), then by type name ascending.
        // Matches C++ Hf_PluginEntry::operator< which sorts lower priority first.
        // The "default" plugin is get_plugin_descs()[0] = lowest numeric priority.
        match self.priority.cmp(&other.priority) {
            std::cmp::Ordering::Equal => self.type_name.cmp(&other.type_name),
            ord => ord,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::any::Any;

    struct TestPlugin {
        _name: String,
    }

    impl TestPlugin {
        fn new(name: &str) -> Self {
            Self {
                _name: name.to_string(),
            }
        }
    }

    impl HfPluginBase for TestPlugin {
        fn type_name(&self) -> &'static str {
            "TestPlugin"
        }

        fn as_any(&self) -> &dyn Any {
            self
        }

        fn as_any_mut(&mut self) -> &mut dyn Any {
            self
        }
    }

    fn create_test_entry() -> HfPluginEntry {
        HfPluginEntry::new(
            TypeId::of::<TestPlugin>(),
            "TestPlugin".to_string(),
            "Test Plugin".to_string(),
            100,
            Box::new(|| Box::new(TestPlugin::new("test"))),
        )
    }

    #[test]
    fn test_plugin_entry_creation() {
        let entry = create_test_entry();

        assert_eq!(entry.type_name(), "TestPlugin");
        assert_eq!(entry.display_name(), "Test Plugin");
        assert_eq!(entry.priority(), 100);
        assert_eq!(entry.ref_count(), 0);
        assert!(!entry.is_loaded());
    }

    #[test]
    fn test_plugin_entry_ref_counting() {
        let entry = create_test_entry();

        assert_eq!(entry.ref_count(), 0);
        assert!(!entry.is_loaded());

        entry.inc_ref_count();
        assert_eq!(entry.ref_count(), 1);
        assert!(entry.is_loaded());

        entry.inc_ref_count();
        assert_eq!(entry.ref_count(), 2);
        assert!(entry.is_loaded());

        entry.dec_ref_count();
        assert_eq!(entry.ref_count(), 1);
        assert!(entry.is_loaded());

        entry.dec_ref_count();
        assert_eq!(entry.ref_count(), 0);
        assert!(!entry.is_loaded());
    }

    #[test]
    fn test_plugin_entry_lazy_instantiation() {
        let entry = create_test_entry();

        // Initially no instance
        assert!(!entry.is_loaded());
        assert!(entry.instance().is_none());

        // Increment ref count creates instance
        entry.inc_ref_count();
        assert!(entry.is_loaded());
        assert!(entry.instance().is_some());
    }

    #[test]
    fn test_plugin_entry_get_desc() {
        let entry = create_test_entry();
        let desc = entry.get_desc();

        assert_eq!(desc.id.as_str(), "TestPlugin");
        assert_eq!(desc.display_name, "Test Plugin");
        assert_eq!(desc.priority, 100);
    }

    #[test]
    fn test_plugin_entry_ordering() {
        let high = HfPluginEntry::new(
            TypeId::of::<TestPlugin>(),
            "HighPriority".to_string(),
            "High".to_string(),
            100,
            Box::new(|| Box::new(TestPlugin::new("high"))),
        );

        let low = HfPluginEntry::new(
            TypeId::of::<TestPlugin>(),
            "LowPriority".to_string(),
            "Low".to_string(),
            10,
            Box::new(|| Box::new(TestPlugin::new("low"))),
        );

        // Lower numeric priority sorts first (matches C++ operator< ascending order).
        assert!(low < high);
    }

    #[test]
    fn test_plugin_entry_equality() {
        let entry1 = create_test_entry();
        let entry2 = create_test_entry();

        // Same type ID means equal
        assert!(entry1 == entry2);
    }
}
