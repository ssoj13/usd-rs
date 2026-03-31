//! Accessibility API schema.
//!
//! Provides accessibility information for assistive technologies.
//! This is a multiple-apply API schema with namespaced instances.
//!
//! # C++ Reference
//!
//! Port of `pxr/usd/usdUI/accessibilityAPI.h`

use std::sync::Arc;

use usd_core::{Attribute, Prim, SchemaKind, Stage};
use usd_sdf::{Path, ValueTypeRegistry};
use usd_tf::Token;
use usd_vt::Value;

use super::tokens::USD_UI_TOKENS;

/// Priority level for accessibility information.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Priority {
    /// Low priority.
    Low,
    /// Standard priority (default).
    #[default]
    Standard,
    /// High priority.
    High,
}

impl Priority {
    /// Converts to token.
    pub fn to_token(&self) -> Token {
        match self {
            Priority::Low => USD_UI_TOKENS.low.clone(),
            Priority::Standard => USD_UI_TOKENS.standard.clone(),
            Priority::High => USD_UI_TOKENS.high.clone(),
        }
    }

    /// Parses from token.
    pub fn from_token(token: &Token) -> Option<Self> {
        match token.as_str() {
            "low" => Some(Priority::Low),
            "standard" => Some(Priority::Standard),
            "high" => Some(Priority::High),
            _ => None,
        }
    }
}

/// Accessibility API schema (multiple-apply).
///
/// Provides accessibility information (label, description, priority) for
/// assistive technologies like screen readers and voice controls.
///
/// # Schema Kind
///
/// This is a multiple-apply API schema (MultipleApplyAPI). Each instance
/// is identified by a name (e.g., "default", "color", "size").
///
/// # Best Practices
///
/// - Use "default" instance name for critical information
/// - Author default values for time-sampled accessibility info
/// - Provide accessibility on default prim and top-level prims
///
/// # Attributes (per instance)
///
/// - `accessibility:<name>:label` - Short concise label
/// - `accessibility:<name>:description` - Extended description
/// - `accessibility:<name>:priority` - Priority hint (low, standard, high)
#[derive(Debug, Clone)]
pub struct AccessibilityAPI {
    prim: Prim,
    instance_name: Token,
}

impl AccessibilityAPI {
    /// The schema kind for this class.
    pub const SCHEMA_KIND: SchemaKind = SchemaKind::MultipleApplyAPI;

    /// The schema type name.
    pub const SCHEMA_TYPE_NAME: &'static str = "AccessibilityAPI";

    /// The property namespace prefix.
    pub const NAMESPACE_PREFIX: &'static str = "accessibility:";

    /// Default instance name for primary accessibility info.
    pub const DEFAULT_INSTANCE_NAME: &'static str = "default";

    // =========================================================================
    // Construction
    // =========================================================================

    /// Construct an AccessibilityAPI on the given prim with instance name.
    pub fn new(prim: Prim, instance_name: Token) -> Self {
        Self {
            prim,
            instance_name,
        }
    }

    /// Construct from another prim with instance name.
    pub fn from_prim(prim: &Prim, instance_name: &Token) -> Self {
        Self::new(prim.clone(), instance_name.clone())
    }

    /// Return an AccessibilityAPI holding the prim at `path` on `stage`
    /// with the given instance name.
    pub fn get(stage: &Arc<Stage>, path: &Path, instance_name: &Token) -> Option<Self> {
        let prim = stage.get_prim_at_path(path)?;
        let api_name = Self::make_api_name(instance_name);
        if prim.has_api(&api_name) {
            Some(Self::new(prim, instance_name.clone()))
        } else {
            None
        }
    }

    /// Returns true if this API schema can be applied to the given prim.
    pub fn can_apply(prim: &Prim, instance_name: &Token, _why_not: Option<&mut String>) -> bool {
        let api_name = Self::make_api_name(instance_name);
        prim.can_apply_api(&api_name)
    }

    /// Applies this API schema to the given prim with instance name.
    ///
    /// Adds "AccessibilityAPI:<name>" to the prim's apiSchemas metadata.
    pub fn apply(prim: &Prim, instance_name: &Token) -> Option<Self> {
        let api_name = Self::make_api_name(instance_name);
        if prim.apply_api(&api_name) {
            Some(Self::new(prim.clone(), instance_name.clone()))
        } else {
            None
        }
    }

    /// Create an AccessibilityAPI with the default instance name.
    ///
    /// Matches C++ `UsdUIAccessibilityAPI::CreateDefaultAPI(const UsdPrim&)`.
    pub fn create_default_api(prim: &Prim) -> Self {
        Self::new(prim.clone(), Token::new(Self::DEFAULT_INSTANCE_NAME))
    }

    /// Create an AccessibilityAPI from any schema object using the default instance name.
    ///
    /// Matches C++ `UsdUIAccessibilityAPI::CreateDefaultAPI(const UsdSchemaBase&)`.
    pub fn create_default_api_from_schema(schema_prim: &Prim) -> Self {
        Self::new(schema_prim.clone(), Token::new(Self::DEFAULT_INSTANCE_NAME))
    }

    /// Apply an AccessibilityAPI with the default instance name.
    pub fn apply_default_api(prim: &Prim) -> Option<Self> {
        Self::apply(prim, &Token::new(Self::DEFAULT_INSTANCE_NAME))
    }

    /// Returns all AccessibilityAPI instances on the given prim.
    pub fn get_all(prim: &Prim) -> Vec<Self> {
        let mut results = Vec::new();
        for api_name in prim.get_applied_schemas() {
            let name_str = api_name.as_str();
            if let Some(instance) = name_str.strip_prefix("AccessibilityAPI:") {
                results.push(Self::new(prim.clone(), Token::new(instance)));
            }
        }
        results
    }

    /// Returns the schema kind.
    pub fn get_schema_kind(&self) -> SchemaKind {
        Self::SCHEMA_KIND
    }

    /// Returns true if this schema is valid.
    pub fn is_valid(&self) -> bool {
        self.prim.is_valid()
    }

    /// Get the wrapped prim.
    pub fn get_prim(&self) -> &Prim {
        &self.prim
    }

    /// Get the instance name.
    pub fn get_name(&self) -> &Token {
        &self.instance_name
    }

    // =========================================================================
    // Label Attribute
    // =========================================================================

    /// Get the label attribute for this instance.
    ///
    /// A short label to concisely describe the prim.
    ///
    /// | Property | Value |
    /// |----------|-------|
    /// | Declaration | `string accessibility:<name>:label` |
    /// | C++ Type | std::string |
    pub fn get_label_attr(&self) -> Option<Attribute> {
        let attr_name = self.make_attr_name(&USD_UI_TOKENS.label);
        self.prim.get_attribute(&attr_name)
    }

    /// Creates the label attribute for this instance.
    pub fn create_label_attr(
        &self,
        default_value: Option<&Value>,
        _write_sparsely: bool,
    ) -> Attribute {
        if !self.prim.is_valid() {
            return Attribute::invalid();
        }

        let attr_name = self.make_attr_name(&USD_UI_TOKENS.label);

        let registry = ValueTypeRegistry::instance();
        let string_type = registry.find_type_by_token(&Token::new("string"));

        let attr = self
            .prim
            .create_attribute(&attr_name, &string_type, false, None)
            .unwrap_or_else(Attribute::invalid);
        if let Some(val) = default_value {
            attr.set(val.clone(), usd_sdf::TimeCode::default());
        }
        attr
    }

    // =========================================================================
    // Description Attribute
    // =========================================================================

    /// Get the description attribute for this instance.
    ///
    /// Extended description of the prim with more details.
    /// May be time-varying for runtimes that support it.
    ///
    /// | Property | Value |
    /// |----------|-------|
    /// | Declaration | `string accessibility:<name>:description` |
    /// | C++ Type | std::string |
    pub fn get_description_attr(&self) -> Option<Attribute> {
        let attr_name = self.make_attr_name(&USD_UI_TOKENS.description);
        self.prim.get_attribute(&attr_name)
    }

    /// Creates the description attribute for this instance.
    pub fn create_description_attr(
        &self,
        default_value: Option<&Value>,
        _write_sparsely: bool,
    ) -> Attribute {
        if !self.prim.is_valid() {
            return Attribute::invalid();
        }

        let attr_name = self.make_attr_name(&USD_UI_TOKENS.description);

        let registry = ValueTypeRegistry::instance();
        let string_type = registry.find_type_by_token(&Token::new("string"));

        let attr = self
            .prim
            .create_attribute(&attr_name, &string_type, false, None)
            .unwrap_or_else(Attribute::invalid);
        if let Some(val) = default_value {
            attr.set(val.clone(), usd_sdf::TimeCode::default());
        }
        attr
    }

    // =========================================================================
    // Priority Attribute
    // =========================================================================

    /// Get the priority attribute for this instance.
    ///
    /// Priority hint for how to rank this accessibility info.
    /// Allowed values: low, standard, high (default: standard)
    ///
    /// | Property | Value |
    /// |----------|-------|
    /// | Declaration | `token accessibility:<name>:priority = "standard"` |
    /// | C++ Type | TfToken |
    /// | Allowed Values | low, standard, high |
    pub fn get_priority_attr(&self) -> Option<Attribute> {
        let attr_name = self.make_attr_name(&USD_UI_TOKENS.priority);
        self.prim.get_attribute(&attr_name)
    }

    /// Creates the priority attribute for this instance.
    pub fn create_priority_attr(
        &self,
        default_value: Option<&Value>,
        _write_sparsely: bool,
    ) -> Attribute {
        if !self.prim.is_valid() {
            return Attribute::invalid();
        }

        let attr_name = self.make_attr_name(&USD_UI_TOKENS.priority);

        let registry = ValueTypeRegistry::instance();
        let token_type = registry.find_type_by_token(&Token::new("token"));

        let attr = self
            .prim
            .create_attribute(&attr_name, &token_type, false, None)
            .unwrap_or_else(Attribute::invalid);
        if let Some(val) = default_value {
            attr.set(val.clone(), usd_sdf::TimeCode::default());
        }
        attr
    }

    // =========================================================================
    // Helper methods
    // =========================================================================

    /// Make the full API schema name with instance.
    fn make_api_name(instance_name: &Token) -> Token {
        Token::new(&format!("AccessibilityAPI:{}", instance_name.as_str()))
    }

    /// Make the full attribute name with instance namespace.
    fn make_attr_name(&self, base_name: &Token) -> String {
        format!(
            "{}{}:{}",
            Self::NAMESPACE_PREFIX,
            self.instance_name.as_str(),
            base_name.as_str()
        )
    }

    // =========================================================================
    // Schema attribute names
    // =========================================================================

    /// Returns all pre-declared attributes for this schema class.
    pub fn get_schema_attribute_names(_include_inherited: bool) -> Vec<Token> {
        vec![
            USD_UI_TOKENS.label.clone(),
            USD_UI_TOKENS.description.clone(),
            USD_UI_TOKENS.priority.clone(),
        ]
    }

    /// Returns attribute names with proper namespace for a given instance.
    pub fn get_schema_attribute_names_for_instance(
        include_inherited: bool,
        instance_name: &Token,
    ) -> Vec<Token> {
        Self::get_schema_attribute_names(include_inherited)
            .into_iter()
            .map(|base| {
                Token::new(&format!(
                    "{}{}:{}",
                    Self::NAMESPACE_PREFIX,
                    instance_name.as_str(),
                    base.as_str()
                ))
            })
            .collect()
    }

    /// Check if a base name is a property of this schema.
    pub fn is_schema_property_base_name(base_name: &Token) -> bool {
        matches!(base_name.as_str(), "label" | "description" | "priority")
    }
}

impl From<(Prim, Token)> for AccessibilityAPI {
    fn from((prim, instance_name): (Prim, Token)) -> Self {
        Self::new(prim, instance_name)
    }
}

impl AsRef<Prim> for AccessibilityAPI {
    fn as_ref(&self) -> &Prim {
        &self.prim
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_schema_kind() {
        assert_eq!(AccessibilityAPI::SCHEMA_KIND, SchemaKind::MultipleApplyAPI);
    }

    #[test]
    fn test_schema_type_name() {
        assert_eq!(AccessibilityAPI::SCHEMA_TYPE_NAME, "AccessibilityAPI");
    }

    #[test]
    fn test_default_instance_name() {
        assert_eq!(AccessibilityAPI::DEFAULT_INSTANCE_NAME, "default");
    }

    #[test]
    fn test_make_api_name() {
        let name = AccessibilityAPI::make_api_name(&Token::new("color"));
        assert_eq!(name.as_str(), "AccessibilityAPI:color");
    }

    #[test]
    fn test_schema_attribute_names() {
        let names = AccessibilityAPI::get_schema_attribute_names(false);
        assert!(names.iter().any(|n| n == "label"));
        assert!(names.iter().any(|n| n == "description"));
        assert!(names.iter().any(|n| n == "priority"));
    }

    #[test]
    fn test_is_schema_property_base_name() {
        assert!(AccessibilityAPI::is_schema_property_base_name(&Token::new(
            "label"
        )));
        assert!(AccessibilityAPI::is_schema_property_base_name(&Token::new(
            "description"
        )));
        assert!(!AccessibilityAPI::is_schema_property_base_name(
            &Token::new("unknown")
        ));
    }
}
