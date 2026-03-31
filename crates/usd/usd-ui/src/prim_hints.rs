//! UsdUIPrimHints - UI hints for UsdPrim.
//!
//! Port of pxr/usd/usdUI/primHints.h/cpp
//!
//! A "schema-like" wrapper that provides API for retrieving and authoring
//! UI hint values within the `uiHints` dictionary metadata field on a UsdPrim.
//!
//! Extends ObjectHints with display group expansion and display group shownIf.

use std::collections::HashMap;

use usd_core::Prim;
use usd_tf::Token;
use usd_vt::{Dictionary, Value};

use super::object_hints::{HintKeys, make_key_path};

/// UI hints wrapper for UsdPrim.
///
/// Provides display group expansion and "shown if" dictionaries via the
/// `uiHints` dictionary metadata, in addition to ObjectHints functionality
/// (display name, hidden).
///
/// Matches C++ `UsdUIPrimHints`.
#[derive(Debug, Clone)]
pub struct PrimHints {
    prim: Prim,
}

impl PrimHints {
    /// Creates an invalid hints object.
    pub fn new() -> Self {
        Self {
            prim: Prim::invalid(),
        }
    }

    /// Constructs a hints object for the given prim.
    pub fn from_prim(prim: Prim) -> Self {
        Self { prim }
    }

    /// Returns the underlying prim.
    pub fn prim(&self) -> &Prim {
        &self.prim
    }

    /// Returns true if valid.
    pub fn is_valid(&self) -> bool {
        self.prim.is_valid()
    }

    // --- ObjectHints-equivalent methods (display name, hidden) ---

    /// Returns the display name from uiHints, falling back to legacy metadata.
    pub fn get_display_name(&self) -> String {
        if !self.prim.is_valid() {
            return String::new();
        }
        if let Some(val) = self
            .prim
            .get_metadata_by_dict_key(&HintKeys::ui_hints(), &HintKeys::display_name())
        {
            if let Some(s) = val.get::<String>() {
                return s.clone();
            }
        }
        // Legacy fallback
        self.prim
            .get_metadata::<String>(&Token::new("displayName"))
            .unwrap_or_default()
    }

    /// Sets display name in uiHints dictionary.
    pub fn set_display_name(&self, name: &str) -> bool {
        if !self.prim.is_valid() {
            return false;
        }
        self.prim.set_metadata_by_dict_key(
            &HintKeys::ui_hints(),
            &HintKeys::display_name(),
            Value::new(name.to_string()),
        )
    }

    /// Returns the hidden status from uiHints.
    pub fn get_hidden(&self) -> bool {
        if !self.prim.is_valid() {
            return false;
        }
        if let Some(val) = self
            .prim
            .get_metadata_by_dict_key(&HintKeys::ui_hints(), &HintKeys::hidden())
        {
            if let Some(&b) = val.get::<bool>() {
                return b;
            }
        }
        // Legacy fallback
        self.prim
            .get_metadata::<bool>(&Token::new("hidden"))
            .unwrap_or(false)
    }

    /// Sets hidden in uiHints dictionary.
    pub fn set_hidden(&self, hidden: bool) -> bool {
        if !self.prim.is_valid() {
            return false;
        }
        self.prim.set_metadata_by_dict_key(
            &HintKeys::ui_hints(),
            &HintKeys::hidden(),
            Value::from(hidden),
        )
    }

    // --- PrimHints-specific methods ---

    /// Returns the prim's display group expansion dictionary.
    ///
    /// Keys are group names, values are booleans indicating expanded state.
    pub fn get_display_groups_expanded(&self) -> Dictionary {
        if !self.prim.is_valid() {
            return Dictionary::new();
        }
        if let Some(val) = self
            .prim
            .get_metadata_by_dict_key(&HintKeys::ui_hints(), &HintKeys::display_groups_expanded())
        {
            if let Some(d) = val.get::<Dictionary>() {
                return d.clone();
            }
            // Also try HashMap<String, Value>
            if let Some(d) = val.get::<HashMap<String, Value>>() {
                let mut dict = Dictionary::new();
                for (k, v) in d {
                    dict.insert(k.clone(), v.clone());
                }
                return dict;
            }
        }
        Dictionary::new()
    }

    /// Sets the prim's display group expansion dictionary.
    ///
    /// All entries must be boolean values.
    pub fn set_display_groups_expanded(&self, expanded: &Dictionary) -> bool {
        if !self.prim.is_valid() {
            return false;
        }
        // Verify all entries are booleans
        for (_, v) in expanded.iter() {
            if v.get::<bool>().is_none() {
                return false;
            }
        }
        self.prim.set_metadata_by_dict_key(
            &HintKeys::ui_hints(),
            &HintKeys::display_groups_expanded(),
            Value::from(expanded.clone()),
        )
    }

    /// Returns whether the named display group should be expanded.
    pub fn get_display_group_expanded(&self, group: &str) -> bool {
        let expanded = self.get_display_groups_expanded();
        expanded
            .get(group)
            .and_then(|v| v.get::<bool>().copied())
            .unwrap_or(false)
    }

    /// Sets whether the named display group should be expanded.
    ///
    /// Uses whole-dictionary approach to avoid colon-nesting issues.
    pub fn set_display_group_expanded(&self, group: &str, expanded: bool) -> bool {
        if !self.prim.is_valid() {
            return false;
        }
        let mut dict = self.get_display_groups_expanded();
        dict.insert(group.to_string(), Value::from(expanded));
        self.set_display_groups_expanded(&dict)
    }

    /// Returns the display group "shown if" dictionary.
    ///
    /// Keys are group names, values are expression strings.
    pub fn get_display_groups_shown_if(&self) -> Dictionary {
        if !self.prim.is_valid() {
            return Dictionary::new();
        }
        if let Some(val) = self
            .prim
            .get_metadata_by_dict_key(&HintKeys::ui_hints(), &HintKeys::display_groups_shown_if())
        {
            if let Some(d) = val.get::<Dictionary>() {
                return d.clone();
            }
            if let Some(d) = val.get::<HashMap<String, Value>>() {
                let mut dict = Dictionary::new();
                for (k, v) in d {
                    dict.insert(k.clone(), v.clone());
                }
                return dict;
            }
        }
        Dictionary::new()
    }

    /// Sets the display group "shown if" dictionary.
    ///
    /// All entries must be string values.
    pub fn set_display_groups_shown_if(&self, shown_if: &Dictionary) -> bool {
        if !self.prim.is_valid() {
            return false;
        }
        // Verify all entries are strings
        for (_, v) in shown_if.iter() {
            if v.get::<String>().is_none() {
                return false;
            }
        }
        self.prim.set_metadata_by_dict_key(
            &HintKeys::ui_hints(),
            &HintKeys::display_groups_shown_if(),
            Value::from(shown_if.clone()),
        )
    }

    /// Returns the "shown if" expression for the named group.
    pub fn get_display_group_shown_if(&self, group: &str) -> String {
        if !self.prim.is_valid() {
            return String::new();
        }
        let key_path = make_key_path(&HintKeys::display_groups_shown_if(), &Token::new(group));
        if let Some(val) = self
            .prim
            .get_metadata_by_dict_key(&HintKeys::ui_hints(), &key_path)
        {
            if let Some(s) = val.get::<String>() {
                return s.clone();
            }
        }
        String::new()
    }

    /// Sets the "shown if" expression for the named group.
    pub fn set_display_group_shown_if(&self, group: &str, shown_if: &str) -> bool {
        if !self.prim.is_valid() {
            return false;
        }
        let key_path = make_key_path(&HintKeys::display_groups_shown_if(), &Token::new(group));
        self.prim.set_metadata_by_dict_key(
            &HintKeys::ui_hints(),
            &key_path,
            Value::new(shown_if.to_string()),
        )
    }
}

impl Default for PrimHints {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_prim_hints() {
        let hints = PrimHints::new();
        assert!(!hints.is_valid());
        assert_eq!(hints.get_display_name(), "");
        assert!(!hints.get_hidden());
        assert!(hints.get_display_groups_expanded().is_empty());
        assert!(hints.get_display_groups_shown_if().is_empty());
    }

    #[test]
    fn test_invalid_set_returns_false() {
        let hints = PrimHints::new();
        assert!(!hints.set_display_name("test"));
        assert!(!hints.set_hidden(true));
        assert!(!hints.set_display_group_expanded("grp", true));
        assert!(!hints.set_display_group_shown_if("grp", "expr"));
    }

    #[test]
    fn test_display_group_expanded_default() {
        let hints = PrimHints::new();
        assert!(!hints.get_display_group_expanded("SomeGroup"));
    }

    #[test]
    fn test_display_group_shown_if_default() {
        let hints = PrimHints::new();
        assert_eq!(hints.get_display_group_shown_if("SomeGroup"), "");
    }
}
