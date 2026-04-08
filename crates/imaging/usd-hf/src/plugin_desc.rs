//! HfPluginDesc - Plugin descriptor structure.
//!
//! Common structure used to report registered plugins in plugin registries.

use std::cmp::Ordering;
use usd_tf::Token;

/// Descriptor for a registered plugin.
///
/// # Fields
///
/// * `id` - Token used for internal API communication about the plugin name
/// * `display_name` - Human-readable name for UI/menus
/// * `priority` - Ordering value; higher priority = higher precedence
///
/// # Ordering
///
/// Plugins are ordered by priority (descending), then alphabetically by id.
/// The plugin with the highest priority is considered the default.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HfPluginDesc {
    /// Token used for internal API communication about the plugin name
    pub id: Token,

    /// Human-readable name for UI/menus
    pub display_name: String,

    /// Ordering value; higher priority = higher precedence
    pub priority: i32,
}

impl HfPluginDesc {
    /// Creates a new plugin descriptor.
    pub fn new(id: Token, display_name: String, priority: i32) -> Self {
        Self {
            id,
            display_name,
            priority,
        }
    }
}

impl Ord for HfPluginDesc {
    fn cmp(&self, other: &Self) -> Ordering {
        // Sort by priority ASCENDING (lower number = sorts first), matching C++ operator<.
        // This matches Hf_PluginEntry::operator< in pluginEntry.cpp.
        match self.priority.cmp(&other.priority) {
            Ordering::Equal => self.id.as_str().cmp(other.id.as_str()),
            ord => ord,
        }
    }
}

impl PartialOrd for HfPluginDesc {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Vector of plugin descriptors.
pub type HfPluginDescVector = Vec<HfPluginDesc>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plugin_desc_creation() {
        let desc = HfPluginDesc::new(Token::new("TestPlugin"), "Test Plugin".to_string(), 100);

        assert_eq!(desc.id.as_str(), "TestPlugin");
        assert_eq!(desc.display_name, "Test Plugin");
        assert_eq!(desc.priority, 100);
    }

    #[test]
    fn test_plugin_desc_ordering_by_priority() {
        let high = HfPluginDesc::new(Token::new("High"), "High Priority".to_string(), 100);
        let low = HfPluginDesc::new(Token::new("Low"), "Low Priority".to_string(), 10);

        // Lower numeric priority sorts first (matches C++ operator< ascending order).
        // Priority=10 comes before priority=100.
        assert!(low < high);
    }

    #[test]
    fn test_plugin_desc_ordering_by_name() {
        let plugin_a = HfPluginDesc::new(Token::new("PluginA"), "Plugin A".to_string(), 50);
        let plugin_b = HfPluginDesc::new(Token::new("PluginB"), "Plugin B".to_string(), 50);

        // Same priority, alphabetical order
        assert!(plugin_a < plugin_b);
    }

    #[test]
    fn test_plugin_desc_vector() {
        let mut plugins = HfPluginDescVector::new();

        plugins.push(HfPluginDesc::new(Token::new("Low"), "Low".to_string(), 10));
        plugins.push(HfPluginDesc::new(
            Token::new("High"),
            "High".to_string(),
            100,
        ));
        plugins.push(HfPluginDesc::new(
            Token::new("Medium"),
            "Medium".to_string(),
            50,
        ));

        // Sort and verify order: ascending by priority (10 < 50 < 100)
        plugins.sort();

        assert_eq!(plugins[0].id.as_str(), "Low");
        assert_eq!(plugins[1].id.as_str(), "Medium");
        assert_eq!(plugins[2].id.as_str(), "High");
    }

    #[test]
    fn test_plugin_desc_equality() {
        let desc1 = HfPluginDesc::new(Token::new("Test"), "Test".to_string(), 50);
        let desc2 = HfPluginDesc::new(Token::new("Test"), "Test".to_string(), 50);

        assert_eq!(desc1, desc2);
    }
}
