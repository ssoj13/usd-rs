//! UsdUIAttributeHints - UI hints for UsdAttribute.
//!
//! Port of pxr/usd/usdUI/attributeHints.h/cpp
//!
//! A "schema-like" wrapper that provides API for retrieving and authoring
//! UI hint values within the `uiHints` dictionary metadata field on a UsdAttribute.
//!
//! Extends PropertyHints with value labels and value labels order.

use usd_core::Attribute;
use usd_tf::Token;
use usd_vt::{Dictionary, Value};

use super::object_hints::{HintKeys, make_key_path};

/// UI hints wrapper for UsdAttribute.
///
/// Provides value labels and value labels order access via the `uiHints`
/// dictionary metadata, in addition to PropertyHints functionality
/// (display name, hidden, display group, shown if).
///
/// Matches C++ `UsdUIAttributeHints`.
#[derive(Debug, Clone)]
pub struct AttributeHints {
    attr: Attribute,
}

impl AttributeHints {
    /// Creates an invalid hints object.
    pub fn new() -> Self {
        Self {
            attr: Attribute::invalid(),
        }
    }

    /// Constructs a hints object for the given attribute.
    pub fn from_attribute(attr: Attribute) -> Self {
        Self { attr }
    }

    /// Returns the underlying attribute.
    pub fn attribute(&self) -> &Attribute {
        &self.attr
    }

    /// Returns true if valid.
    pub fn is_valid(&self) -> bool {
        self.attr.is_valid()
    }

    // --- ObjectHints-equivalent methods ---

    /// Returns display name from uiHints, falling back to legacy metadata.
    pub fn get_display_name(&self) -> String {
        if !self.attr.is_valid() {
            return String::new();
        }
        if let Some(val) = self
            .attr
            .get_metadata_by_dict_key(&HintKeys::ui_hints(), &HintKeys::display_name())
        {
            if let Some(s) = val.get::<String>() {
                return s.clone();
            }
        }
        self.attr
            .get_metadata(&Token::new("displayName"))
            .and_then(|v| v.get::<String>().cloned())
            .unwrap_or_default()
    }

    /// Sets display name in uiHints.
    pub fn set_display_name(&self, name: &str) -> bool {
        if !self.attr.is_valid() {
            return false;
        }
        self.attr.set_metadata_by_dict_key(
            &HintKeys::ui_hints(),
            &HintKeys::display_name(),
            Value::new(name.to_string()),
        )
    }

    /// Returns hidden status from uiHints.
    pub fn get_hidden(&self) -> bool {
        if !self.attr.is_valid() {
            return false;
        }
        if let Some(val) = self
            .attr
            .get_metadata_by_dict_key(&HintKeys::ui_hints(), &HintKeys::hidden())
        {
            if let Some(&b) = val.get::<bool>() {
                return b;
            }
        }
        self.attr
            .get_metadata(&Token::new("hidden"))
            .and_then(|v| v.get::<bool>().copied())
            .unwrap_or(false)
    }

    /// Sets hidden in uiHints.
    pub fn set_hidden(&self, hidden: bool) -> bool {
        if !self.attr.is_valid() {
            return false;
        }
        self.attr.set_metadata_by_dict_key(
            &HintKeys::ui_hints(),
            &HintKeys::hidden(),
            Value::from(hidden),
        )
    }

    // --- PropertyHints-equivalent methods ---

    /// Returns display group from uiHints, falling back to legacy metadata.
    pub fn get_display_group(&self) -> String {
        if !self.attr.is_valid() {
            return String::new();
        }
        if let Some(val) = self
            .attr
            .get_metadata_by_dict_key(&HintKeys::ui_hints(), &HintKeys::display_group())
        {
            if let Some(s) = val.get::<String>() {
                return s.clone();
            }
        }
        self.attr
            .get_metadata(&Token::new("displayGroup"))
            .and_then(|v| v.get::<String>().cloned())
            .unwrap_or_default()
    }

    /// Sets display group in uiHints.
    pub fn set_display_group(&self, group: &str) -> bool {
        if !self.attr.is_valid() {
            return false;
        }
        self.attr.set_metadata_by_dict_key(
            &HintKeys::ui_hints(),
            &HintKeys::display_group(),
            Value::new(group.to_string()),
        )
    }

    /// Returns "shown if" expression from uiHints.
    pub fn get_shown_if(&self) -> String {
        if !self.attr.is_valid() {
            return String::new();
        }
        if let Some(val) = self
            .attr
            .get_metadata_by_dict_key(&HintKeys::ui_hints(), &HintKeys::shown_if())
        {
            if let Some(s) = val.get::<String>() {
                return s.clone();
            }
        }
        String::new()
    }

    /// Sets "shown if" expression in uiHints.
    pub fn set_shown_if(&self, shown_if: &str) -> bool {
        if !self.attr.is_valid() {
            return false;
        }
        self.attr.set_metadata_by_dict_key(
            &HintKeys::ui_hints(),
            &HintKeys::shown_if(),
            Value::new(shown_if.to_string()),
        )
    }

    // --- AttributeHints-specific methods ---

    /// Returns the attribute's value labels dictionary.
    ///
    /// Maps user-facing label strings to underlying values for the attribute.
    pub fn get_value_labels(&self) -> Dictionary {
        if !self.attr.is_valid() {
            return Dictionary::new();
        }
        if let Some(val) = self
            .attr
            .get_metadata_by_dict_key(&HintKeys::ui_hints(), &HintKeys::value_labels())
        {
            if let Some(d) = val.get::<Dictionary>() {
                return d.clone();
            }
        }
        Dictionary::new()
    }

    /// Sets the attribute's value labels dictionary.
    pub fn set_value_labels(&self, labels: &Dictionary) -> bool {
        if !self.attr.is_valid() {
            return false;
        }
        self.attr.set_metadata_by_dict_key(
            &HintKeys::ui_hints(),
            &HintKeys::value_labels(),
            Value::from(labels.clone()),
        )
    }

    /// Returns the value labels order (token array).
    pub fn get_value_labels_order(&self) -> Vec<Token> {
        if !self.attr.is_valid() {
            return Vec::new();
        }
        if let Some(val) = self
            .attr
            .get_metadata_by_dict_key(&HintKeys::ui_hints(), &HintKeys::value_labels_order())
        {
            if let Some(arr) = val.get::<Vec<Token>>() {
                return arr.clone();
            }
            // Try Vec<String> and convert
            if let Some(arr) = val.get::<Vec<String>>() {
                return arr.iter().map(|s| Token::new(s)).collect();
            }
        }
        Vec::new()
    }

    /// Sets the value labels order.
    pub fn set_value_labels_order(&self, order: &[Token]) -> bool {
        if !self.attr.is_valid() {
            return false;
        }
        self.attr.set_metadata_by_dict_key(
            &HintKeys::ui_hints(),
            &HintKeys::value_labels_order(),
            Value::new(order.to_vec()),
        )
    }

    /// Author the value associated with the given label to the attribute.
    ///
    /// Looks up the label in the value labels dictionary. If found,
    /// sets the attribute value to the associated value. Returns false
    /// if the label is not found or the set fails.
    pub fn apply_value_label(&self, label: &str) -> bool {
        if !self.attr.is_valid() {
            return false;
        }
        let key_path = make_key_path(&HintKeys::value_labels(), &Token::new(label));
        if let Some(value) = self
            .attr
            .get_metadata_by_dict_key(&HintKeys::ui_hints(), &key_path)
        {
            return self.attr.set(value, usd_sdf::TimeCode::default());
        }
        false
    }
}

impl Default for AttributeHints {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_attribute_hints() {
        let hints = AttributeHints::new();
        assert!(!hints.is_valid());
        assert_eq!(hints.get_display_name(), "");
        assert_eq!(hints.get_display_group(), "");
        assert_eq!(hints.get_shown_if(), "");
        assert!(!hints.get_hidden());
        assert!(hints.get_value_labels().is_empty());
        assert!(hints.get_value_labels_order().is_empty());
    }

    #[test]
    fn test_invalid_set_returns_false() {
        let hints = AttributeHints::new();
        assert!(!hints.set_display_name("test"));
        assert!(!hints.set_display_group("group"));
        assert!(!hints.set_shown_if("expr"));
        assert!(!hints.set_hidden(true));
        assert!(!hints.set_value_labels(&Dictionary::new()));
        assert!(!hints.set_value_labels_order(&[]));
        assert!(!hints.apply_value_label("label"));
    }
}
