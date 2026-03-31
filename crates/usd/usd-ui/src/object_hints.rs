//! UsdUIObjectHints - UI hints for UsdObject.
//!
//! Port of pxr/usd/usdUI/objectHints.h/cpp
//!
//! A "schema-like" wrapper that provides API for retrieving and authoring
//! UI hint values within the `uiHints` dictionary metadata field on a UsdObject.

use std::sync::OnceLock;

use usd_core::object::Object;
use usd_tf::Token;
use usd_vt::Value;

/// Returns true if legacy UI hints (displayName, displayGroup, hidden) should
/// also be written to deprecated core metadata fields when setting via uiHints.
/// Controlled by USDUI_WRITE_LEGACY_UI_HINTS env var (default: true).
/// Matches C++ TF_DEFINE_ENV_SETTING(USDUI_WRITE_LEGACY_UI_HINTS, true, ...).
pub(crate) fn write_legacy_ui_hints() -> bool {
    static VALUE: OnceLock<bool> = OnceLock::new();
    *VALUE.get_or_init(|| {
        std::env::var("USDUI_WRITE_LEGACY_UI_HINTS")
            .map(|v| !matches!(v.as_str(), "0" | "false" | "False" | "FALSE"))
            .unwrap_or(true)
    })
}

/// Tokens for UI hint dictionary keys.
///
/// Matches C++ `UsdUIHintKeys`.
pub struct HintKeys;

impl HintKeys {
    /// Key for the top-level uiHints dictionary.
    pub fn ui_hints() -> Token {
        Token::new("uiHints")
    }

    /// Key for display name within uiHints.
    pub fn display_name() -> Token {
        Token::new("displayName")
    }

    /// Key for display group within uiHints.
    pub fn display_group() -> Token {
        Token::new("displayGroup")
    }

    /// Key for hidden flag within uiHints.
    pub fn hidden() -> Token {
        Token::new("hidden")
    }

    /// Key for shownIf expression within uiHints.
    pub fn shown_if() -> Token {
        Token::new("shownIf")
    }

    /// Key for value labels dictionary within uiHints.
    pub fn value_labels() -> Token {
        Token::new("valueLabels")
    }

    /// Key for value labels order within uiHints.
    pub fn value_labels_order() -> Token {
        Token::new("valueLabelsOrder")
    }

    /// Key for display groups expanded dictionary within uiHints.
    pub fn display_groups_expanded() -> Token {
        Token::new("displayGroupsExpanded")
    }

    /// Key for display groups shownIf dictionary within uiHints.
    pub fn display_groups_shown_if() -> Token {
        Token::new("displayGroupsShownIf")
    }
}

/// Combine two tokens into a colon-delimited key path.
///
/// Matches C++ `UsdUIObjectHints::_MakeKeyPath()`.
pub fn make_key_path(key1: &Token, key2: &Token) -> Token {
    Token::new(&format!("{}:{}", key1.as_str(), key2.as_str()))
}

/// A "schema-like" wrapper for UI hints on a UsdObject.
///
/// Provides API for display name and hidden status via the `uiHints`
/// dictionary metadata field.
///
/// Matches C++ `UsdUIObjectHints`.
#[derive(Debug, Clone)]
pub struct ObjectHints {
    obj: Object,
}

impl ObjectHints {
    /// Creates an invalid hints object.
    pub fn new() -> Self {
        Self {
            obj: Object::invalid(),
        }
    }

    /// Constructs a hints object for the given UsdObject.
    pub fn from_object(obj: Object) -> Self {
        Self { obj }
    }

    /// Returns the underlying object.
    pub fn object(&self) -> &Object {
        &self.obj
    }

    /// Returns true if this hints object wraps a valid object.
    pub fn is_valid(&self) -> bool {
        self.obj.is_valid()
    }

    /// Returns the object's display name from the uiHints dictionary.
    ///
    /// Falls back to legacy `displayName` metadata if not found in uiHints.
    pub fn get_display_name(&self) -> String {
        if !self.obj.is_valid() {
            return String::new();
        }

        // Try uiHints:displayName
        if let Some(name) = self
            .obj
            .get_metadata_by_dict_key(&HintKeys::ui_hints(), &HintKeys::display_name())
        {
            if let Some(s) = name.get::<String>() {
                return s.clone();
            }
        }

        // Fall back to legacy displayName
        self.obj
            .get_metadata(&Token::new("displayName"))
            .and_then(|v| v.get::<String>().cloned())
            .unwrap_or_default()
    }

    /// Sets the object's display name in the uiHints dictionary.
    ///
    /// Also writes legacy `displayName` metadata when
    /// USDUI_WRITE_LEGACY_UI_HINTS is enabled (default: true).
    /// Matches C++ `UsdUIObjectHints::SetDisplayName()`.
    pub fn set_display_name(&self, name: &str) -> bool {
        if !self.obj.is_valid() {
            log::error!("Invalid object");
            return false;
        }

        if !self.obj.set_metadata_by_dict_key(
            &HintKeys::ui_hints(),
            &HintKeys::display_name(),
            Value::new(name.to_string()),
        ) {
            return false;
        }

        // Also write legacy displayName field when env var is set
        if write_legacy_ui_hints() {
            self.obj
                .set_metadata(&Token::new("displayName"), Value::new(name.to_string()));
        }

        true
    }

    /// Returns the object's hidden status from the uiHints dictionary.
    ///
    /// Falls back to legacy `hidden` metadata if not found in uiHints.
    pub fn get_hidden(&self) -> bool {
        if !self.obj.is_valid() {
            return false;
        }

        // Try uiHints:hidden
        if let Some(val) = self
            .obj
            .get_metadata_by_dict_key(&HintKeys::ui_hints(), &HintKeys::hidden())
        {
            if let Some(&b) = val.get::<bool>() {
                return b;
            }
        }

        // Fall back to legacy hidden
        self.obj.is_hidden()
    }

    /// Sets the object's hidden status in the uiHints dictionary.
    ///
    /// Also writes legacy `hidden` metadata when
    /// USDUI_WRITE_LEGACY_UI_HINTS is enabled (default: true).
    /// Matches C++ `UsdUIObjectHints::SetHidden()`.
    pub fn set_hidden(&self, hidden: bool) -> bool {
        if !self.obj.is_valid() {
            log::error!("Invalid object");
            return false;
        }

        if !self.obj.set_metadata_by_dict_key(
            &HintKeys::ui_hints(),
            &HintKeys::hidden(),
            Value::from(hidden),
        ) {
            return false;
        }

        // Also write legacy hidden field when env var is set
        if write_legacy_ui_hints() {
            self.obj
                .set_metadata(&Token::new("hidden"), Value::from(hidden));
        }

        true
    }
}

impl Default for ObjectHints {
    fn default() -> Self {
        Self::new()
    }
}

impl PartialEq for ObjectHints {
    fn eq(&self, other: &Self) -> bool {
        self.obj.path() == other.obj.path()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_object_hints() {
        let hints = ObjectHints::new();
        assert!(!hints.is_valid());
        assert_eq!(hints.get_display_name(), "");
        assert!(!hints.get_hidden());
    }

    #[test]
    fn test_invalid_set_returns_false() {
        let hints = ObjectHints::new();
        assert!(!hints.set_display_name("test"));
        assert!(!hints.set_hidden(true));
    }

    #[test]
    fn test_hint_keys() {
        assert_eq!(HintKeys::ui_hints().as_str(), "uiHints");
        assert_eq!(HintKeys::display_name().as_str(), "displayName");
        assert_eq!(HintKeys::hidden().as_str(), "hidden");
        assert_eq!(HintKeys::shown_if().as_str(), "shownIf");
        assert_eq!(HintKeys::value_labels().as_str(), "valueLabels");
    }

    #[test]
    fn test_make_key_path() {
        let path = make_key_path(&Token::new("a"), &Token::new("b"));
        assert_eq!(path.as_str(), "a:b");
    }
}
