//! UsdUIPropertyHints - UI hints for UsdProperty.
//!
//! Port of pxr/usd/usdUI/propertyHints.h/cpp
//!
//! A "schema-like" wrapper that provides API for retrieving and authoring
//! UI hint values within the `uiHints` dictionary metadata field on a UsdProperty.
//!
//! Extends ObjectHints with display group and shownIf.

use usd_core::Property;
use usd_tf::Token;
use usd_vt::Value;

use super::object_hints::{HintKeys, write_legacy_ui_hints};

/// UI hints wrapper for UsdProperty.
///
/// Provides display group and "shown if" expression access via the
/// `uiHints` dictionary metadata, in addition to ObjectHints functionality
/// (display name, hidden).
///
/// Matches C++ `UsdUIPropertyHints`.
#[derive(Debug, Clone)]
pub struct PropertyHints {
    /// The property being wrapped.
    prop: Property,
}

impl PropertyHints {
    /// Creates an invalid hints object.
    pub fn new() -> Self {
        Self {
            prop: Property::invalid(),
        }
    }

    /// Constructs a hints object for the given property.
    pub fn from_property(prop: Property) -> Self {
        Self { prop }
    }

    /// Returns the underlying property.
    pub fn property(&self) -> &Property {
        &self.prop
    }

    /// Returns true if valid.
    pub fn is_valid(&self) -> bool {
        self.prop.is_valid()
    }

    // --- ObjectHints-equivalent methods (display name, hidden) ---

    /// Returns the object's display name from the uiHints dictionary.
    ///
    /// Falls back to legacy `displayName` metadata if not found in uiHints.
    pub fn get_display_name(&self) -> String {
        if !self.prop.is_valid() {
            return String::new();
        }
        if let Some(val) = self
            .prop
            .get_metadata_by_dict_key(&HintKeys::ui_hints(), &HintKeys::display_name())
        {
            if let Some(s) = val.get::<String>() {
                return s.clone();
            }
        }
        // Legacy fallback
        self.prop
            .get_metadata(&Token::new("displayName"))
            .and_then(|v| v.get::<String>().cloned())
            .unwrap_or_default()
    }

    /// Sets the object's display name in the uiHints dictionary.
    ///
    /// Also writes legacy `displayName` metadata when
    /// USDUI_WRITE_LEGACY_UI_HINTS is enabled (default: true).
    pub fn set_display_name(&self, name: &str) -> bool {
        if !self.prop.is_valid() {
            log::error!("Invalid object");
            return false;
        }
        if !self.prop.set_metadata_by_dict_key(
            &HintKeys::ui_hints(),
            &HintKeys::display_name(),
            Value::new(name.to_string()),
        ) {
            return false;
        }
        if write_legacy_ui_hints() {
            self.prop
                .set_metadata(&Token::new("displayName"), Value::new(name.to_string()));
        }
        true
    }

    /// Returns the hidden status from uiHints dictionary.
    pub fn get_hidden(&self) -> bool {
        if !self.prop.is_valid() {
            return false;
        }
        if let Some(val) = self
            .prop
            .get_metadata_by_dict_key(&HintKeys::ui_hints(), &HintKeys::hidden())
        {
            if let Some(&b) = val.get::<bool>() {
                return b;
            }
        }
        // Legacy fallback
        self.prop
            .get_metadata(&Token::new("hidden"))
            .and_then(|v| v.get::<bool>().copied())
            .unwrap_or(false)
    }

    /// Sets the hidden status in the uiHints dictionary.
    ///
    /// Also writes legacy `hidden` metadata when
    /// USDUI_WRITE_LEGACY_UI_HINTS is enabled (default: true).
    pub fn set_hidden(&self, hidden: bool) -> bool {
        if !self.prop.is_valid() {
            log::error!("Invalid object");
            return false;
        }
        if !self.prop.set_metadata_by_dict_key(
            &HintKeys::ui_hints(),
            &HintKeys::hidden(),
            Value::from(hidden),
        ) {
            return false;
        }
        if write_legacy_ui_hints() {
            self.prop
                .set_metadata(&Token::new("hidden"), Value::from(hidden));
        }
        true
    }

    // --- PropertyHints-specific methods ---

    /// Returns the property's display group from the uiHints dictionary.
    ///
    /// Falls back to legacy `displayGroup` metadata if not found in uiHints.
    pub fn get_display_group(&self) -> String {
        if !self.prop.is_valid() {
            return String::new();
        }
        if let Some(val) = self
            .prop
            .get_metadata_by_dict_key(&HintKeys::ui_hints(), &HintKeys::display_group())
        {
            if let Some(s) = val.get::<String>() {
                return s.clone();
            }
        }
        // Legacy fallback
        self.prop
            .get_metadata(&Token::new("displayGroup"))
            .and_then(|v| v.get::<String>().cloned())
            .unwrap_or_default()
    }

    /// Sets the property's display group in the uiHints dictionary.
    ///
    /// Also writes legacy `displayGroup` metadata when
    /// USDUI_WRITE_LEGACY_UI_HINTS is enabled (default: true).
    /// Matches C++ `UsdUIPropertyHints::SetDisplayGroup()`.
    pub fn set_display_group(&self, group: &str) -> bool {
        if !self.prop.is_valid() {
            log::error!("Invalid property");
            return false;
        }
        if !self.prop.set_metadata_by_dict_key(
            &HintKeys::ui_hints(),
            &HintKeys::display_group(),
            Value::new(group.to_string()),
        ) {
            return false;
        }
        if write_legacy_ui_hints() {
            self.prop
                .set_metadata(&Token::new("displayGroup"), Value::new(group.to_string()));
        }
        true
    }

    /// Returns the property's "shown if" expression from uiHints.
    pub fn get_shown_if(&self) -> String {
        if !self.prop.is_valid() {
            return String::new();
        }
        if let Some(val) = self
            .prop
            .get_metadata_by_dict_key(&HintKeys::ui_hints(), &HintKeys::shown_if())
        {
            if let Some(s) = val.get::<String>() {
                return s.clone();
            }
        }
        String::new()
    }

    /// Sets the property's "shown if" expression in uiHints.
    pub fn set_shown_if(&self, shown_if: &str) -> bool {
        if !self.prop.is_valid() {
            return false;
        }
        self.prop.set_metadata_by_dict_key(
            &HintKeys::ui_hints(),
            &HintKeys::shown_if(),
            Value::new(shown_if.to_string()),
        )
    }
}

impl Default for PropertyHints {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_property_hints() {
        let hints = PropertyHints::new();
        assert!(!hints.is_valid());
        assert_eq!(hints.get_display_name(), "");
        assert_eq!(hints.get_display_group(), "");
        assert_eq!(hints.get_shown_if(), "");
        assert!(!hints.get_hidden());
    }

    #[test]
    fn test_invalid_set_returns_false() {
        let hints = PropertyHints::new();
        assert!(!hints.set_display_name("test"));
        assert!(!hints.set_display_group("group"));
        assert!(!hints.set_shown_if("expr"));
        assert!(!hints.set_hidden(true));
    }
}
