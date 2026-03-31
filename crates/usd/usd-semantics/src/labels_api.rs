//! Labels API schema.
//!
//! Multi-apply API for attaching semantic labels to prims.
//!
//! # C++ Reference
//!
//! Port of `pxr/usd/usdSemantics/labelsAPI.h` and `labelsAPI.cpp`

use std::sync::Arc;

use usd_core::attribute::Variability;
use usd_core::{Attribute, Prim, SchemaKind, Stage};
use usd_sdf::{Path, ValueTypeRegistry};
use usd_tf::Token;

use super::tokens::USD_SEMANTICS_TOKENS;

/// Multi-apply API for semantic labels.
///
/// Allows attaching named sets of semantic labels to prims.
/// Each instance has a `semantics:labels:<instance>` token array attribute.
///
/// # Schema Kind
///
/// This is a multiple-apply API schema (MultipleApplyAPI).
#[derive(Debug, Clone)]
pub struct LabelsAPI {
    prim: Prim,
    instance_name: Token,
}

impl LabelsAPI {
    /// The schema kind for this class.
    pub const SCHEMA_KIND: SchemaKind = SchemaKind::MultipleApplyAPI;

    /// Schema identifier, matches C++ `UsdSemanticsTokens->SemanticsLabelsAPI`.
    pub const SCHEMA_TYPE_NAME: &'static str = "SemanticsLabelsAPI";

    // =========================================================================
    // Construction
    // =========================================================================

    /// Construct a LabelsAPI on the given prim with instance name.
    ///
    /// Matches C++ `UsdSemanticsLabelsAPI(prim, name)`.
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

    /// Return a LabelsAPI for the prim at a PROPERTY path on the stage.
    ///
    /// The path must be of the form `<PrimPath>.semantics:labels:<name>`.
    /// Validates via `is_semantics_labels_api_path()` and extracts the instance name.
    ///
    /// Matches C++ `UsdSemanticsLabelsAPI::Get(stage, path)`.
    pub fn get(stage: &Arc<Stage>, path: &Path) -> Option<Self> {
        let instance_name = Self::is_semantics_labels_api_path(path)?;
        let prim_path = path.get_prim_path();
        let prim = stage.get_prim_at_path(&prim_path)?;
        Some(Self::new(prim, instance_name))
    }

    /// Return a LabelsAPI wrapping the prim with the given instance name.
    ///
    /// Matches C++ `UsdSemanticsLabelsAPI::Get(prim, name)`.
    pub fn get_from_prim(prim: &Prim, name: &Token) -> Self {
        Self::new(prim.clone(), name.clone())
    }

    /// Applies this API schema to the given prim with instance name.
    ///
    /// Adds `"SemanticsLabelsAPI:<name>"` to apiSchemas metadata.
    ///
    /// Matches C++ `UsdSemanticsLabelsAPI::Apply(prim, name)`.
    pub fn apply(prim: &Prim, instance_name: &Token) -> Option<Self> {
        let schema_name = format!("{}:{}", Self::SCHEMA_TYPE_NAME, instance_name.as_str());
        if prim.apply_api(&Token::new(&schema_name)) {
            Some(Self::new(prim.clone(), instance_name.clone()))
        } else {
            None
        }
    }

    /// Returns true if this API can be applied to the prim with the given name.
    ///
    /// Matches C++ `UsdSemanticsLabelsAPI::CanApply(prim, name, whyNot)`.
    pub fn can_apply(prim: &Prim, instance_name: &Token, _why_not: Option<&mut String>) -> bool {
        let schema_name = format!("{}:{}", Self::SCHEMA_TYPE_NAME, instance_name.as_str());
        prim.can_apply_api(&Token::new(&schema_name))
    }

    /// Returns the schema kind.
    pub fn get_schema_kind(&self) -> SchemaKind {
        Self::SCHEMA_KIND
    }

    /// Returns the instance name (taxonomy).
    pub fn get_instance_name(&self) -> &Token {
        &self.instance_name
    }

    /// Returns the instance name - alias matching C++ `GetName()`.
    pub fn get_name(&self) -> &Token {
        &self.instance_name
    }

    /// Returns true if this schema wraps a valid prim.
    pub fn is_valid(&self) -> bool {
        self.prim.is_valid()
    }

    /// Get the wrapped prim.
    pub fn get_prim(&self) -> &Prim {
        &self.prim
    }

    // =========================================================================
    // Labels Attribute
    // =========================================================================

    /// Build the full attribute name: `semantics:labels:<instance>`.
    ///
    /// Matches C++ `_GetNamespacedPropertyName(GetName(), semanticsLabels_MultipleApplyTemplate_)`:
    /// `MakeMultipleApplyNameInstance("semantics:labels:__INSTANCE_NAME__", instanceName)`
    /// which substitutes `__INSTANCE_NAME__` → `instanceName`.
    fn labels_attr_name(&self) -> String {
        format!(
            "{}:{}",
            USD_SEMANTICS_TOKENS.semantics_labels.as_str(),
            self.instance_name.as_str()
        )
    }

    /// Get the `semantics:labels:<instance>` attribute.
    ///
    /// Matches C++ `UsdSemanticsLabelsAPI::GetLabelsAttr()`.
    pub fn get_labels_attr(&self) -> Option<Attribute> {
        self.prim.get_attribute(&self.labels_attr_name())
    }

    /// Create (or return existing) the labels attribute with `SdfVariabilityVarying`.
    ///
    /// Matches C++ `UsdSemanticsLabelsAPI::CreateLabelsAttr()`.
    pub fn create_labels_attr(&self) -> Attribute {
        if !self.prim.is_valid() {
            return Attribute::invalid();
        }

        let attr_name = self.labels_attr_name();

        let registry = ValueTypeRegistry::instance();
        let token_array_type = registry.find_type_by_token(&Token::new("token[]"));

        // C++ uses SdfVariabilityVarying - labels can have time samples
        self.prim
            .create_attribute(
                &attr_name,
                &token_array_type,
                false,
                Some(Variability::Varying),
            )
            .unwrap_or_else(Attribute::invalid)
    }

    // =========================================================================
    // Static query methods
    // =========================================================================

    /// Returns all LabelsAPI instances applied to the given prim.
    ///
    /// Strips the `"SemanticsLabelsAPI:"` prefix from applied schema names.
    ///
    /// Matches C++ `UsdSemanticsLabelsAPI::GetAll(prim)`.
    pub fn get_all(prim: &Prim) -> Vec<Self> {
        let prefix = format!("{}:", Self::SCHEMA_TYPE_NAME);
        prim.get_applied_schemas()
            .into_iter()
            .filter_map(|api_name| {
                api_name
                    .as_str()
                    .strip_prefix(&prefix)
                    .map(|instance| Self::new(prim.clone(), Token::new(instance)))
            })
            .collect()
    }

    /// Get the taxonomy names of all directly applied LabelsAPI instances.
    ///
    /// Returns empty vec for pseudo-root prims.
    ///
    /// Matches C++ `UsdSemanticsLabelsAPI::GetDirectTaxonomies(prim)`.
    pub fn get_direct_taxonomies(prim: &Prim) -> Vec<Token> {
        // C++ returns {} for pseudo-root
        if prim.is_pseudo_root() {
            return vec![];
        }
        Self::get_all(prim)
            .into_iter()
            .map(|api| api.instance_name)
            .collect()
    }

    /// Compute unique taxonomy names from this prim and all its ancestors.
    ///
    /// Results are sorted. Includes self.
    ///
    /// Matches C++ `UsdSemanticsLabelsAPI::ComputeInheritedTaxonomies(prim)`.
    pub fn compute_inherited_taxonomies(prim: &Prim) -> Vec<Token> {
        use std::collections::HashSet;
        let Some(stage) = prim.stage() else {
            return vec![];
        };

        let mut unique: HashSet<Token> = HashSet::new();

        // GetAncestorsRange includes the prim itself and walks up to root
        for path in prim.get_path().get_ancestors_range() {
            let ancestor = stage.get_prim_at_path(&path);
            if let Some(anc) = ancestor {
                for api in Self::get_all(&anc) {
                    unique.insert(api.instance_name);
                }
            }
        }

        let mut result: Vec<Token> = unique.into_iter().collect();
        result.sort_by(|a, b| a.as_str().cmp(b.as_str()));
        result
    }

    /// Check if a base name is a property base name of this schema.
    ///
    /// The base name for `semantics:labels:__INSTANCE_NAME__` (after stripping
    /// the multiple-apply namespace) is `"__INSTANCE_NAME__"`.
    ///
    /// Matches C++ `UsdSemanticsLabelsAPI::IsSchemaPropertyBaseName(baseName)`.
    pub fn is_schema_property_base_name(base_name: &Token) -> bool {
        // C++ strips namespace from semanticsLabels_MultipleApplyTemplate_
        // ("semantics:labels:__INSTANCE_NAME__") to get "__INSTANCE_NAME__"
        base_name == "__INSTANCE_NAME__"
    }

    /// Check if `path` is a valid SemanticsLabelsAPI property path.
    ///
    /// Returns `Some(instance_name)` if the path is a property path of the form
    /// `<PrimPath>.semantics:labels:<name>`, otherwise `None`.
    ///
    /// Matches C++ `UsdSemanticsLabelsAPI::IsSemanticsLabelsAPIPath(path, &name)`.
    pub fn is_semantics_labels_api_path(path: &Path) -> Option<Token> {
        // Must be a property path
        if !path.is_property_path() {
            return None;
        }

        let property_name = path.get_name();
        // Tokenize by colon: "semantics:labels:foo" -> ["semantics", "labels", "foo"]
        // (or just "semantics:labels" -> ["semantics", "labels"])
        let tokens: Vec<&str> = property_name.split(':').collect();

        // Need at least 3 parts: semantics, labels, <instance>
        if tokens.len() < 3 {
            return None;
        }

        let base_ns = USD_SEMANTICS_TOKENS.semantics_labels.as_str(); // "semantics:labels"
        // Check first two tokens form the namespace prefix
        if format!("{}:{}", tokens[0], tokens[1]) != base_ns {
            return None;
        }

        // The last token is the base name - reject if it's a schema property base name
        let base_name = Token::new(tokens.last().unwrap());
        if Self::is_schema_property_base_name(&base_name) {
            return None;
        }

        // Instance name = everything after "semantics:labels:"
        let prefix_len = base_ns.len() + 1; // +1 for the separating ':'
        let instance_name = &property_name[prefix_len..];
        Some(Token::new(instance_name))
    }

    // =========================================================================
    // Schema attribute names
    // =========================================================================

    /// Returns pre-declared attribute names for this schema.
    ///
    /// Returns the template token `"semantics:labels:__INSTANCE_NAME__"`.
    ///
    /// Matches C++ `UsdSemanticsLabelsAPI::GetSchemaAttributeNames(includeInherited)`.
    pub fn get_schema_attribute_names(_include_inherited: bool) -> Vec<Token> {
        vec![
            USD_SEMANTICS_TOKENS
                .semantics_labels_multiple_apply_template
                .clone(),
        ]
    }

    /// Returns attribute names with proper namespace for a given instance.
    ///
    /// Substitutes `__INSTANCE_NAME__` with the actual instance name.
    ///
    /// Matches C++ `UsdSemanticsLabelsAPI::GetSchemaAttributeNames(includeInherited, instanceName)`.
    pub fn get_schema_attribute_names_for_instance(
        include_inherited: bool,
        instance_name: &Token,
    ) -> Vec<Token> {
        Self::get_schema_attribute_names(include_inherited)
            .into_iter()
            .map(|tmpl| {
                let resolved = tmpl
                    .as_str()
                    .replace("__INSTANCE_NAME__", instance_name.as_str());
                Token::new(&resolved)
            })
            .collect()
    }
}

impl From<LabelsAPI> for Prim {
    fn from(api: LabelsAPI) -> Self {
        api.prim
    }
}

impl AsRef<Prim> for LabelsAPI {
    fn as_ref(&self) -> &Prim {
        &self.prim
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_schema_kind() {
        assert_eq!(LabelsAPI::SCHEMA_KIND, SchemaKind::MultipleApplyAPI);
    }

    #[test]
    fn test_schema_type_name() {
        // P1-7: must be "SemanticsLabelsAPI" not "LabelsAPI"
        assert_eq!(LabelsAPI::SCHEMA_TYPE_NAME, "SemanticsLabelsAPI");
    }

    #[test]
    fn test_get_schema_attribute_names() {
        let names = LabelsAPI::get_schema_attribute_names(false);
        // P1-8: must return template token
        assert_eq!(names.len(), 1);
        assert_eq!(names[0].as_str(), "semantics:labels:__INSTANCE_NAME__");
    }

    #[test]
    fn test_get_schema_attribute_names_for_instance() {
        let names =
            LabelsAPI::get_schema_attribute_names_for_instance(false, &Token::new("training"));
        assert_eq!(names.len(), 1);
        assert_eq!(names[0].as_str(), "semantics:labels:training");
    }

    #[test]
    fn test_is_schema_property_base_name() {
        // P1-8: base name for this multi-apply schema is "__INSTANCE_NAME__"
        assert!(LabelsAPI::is_schema_property_base_name(&Token::new(
            "__INSTANCE_NAME__"
        )));
        assert!(!LabelsAPI::is_schema_property_base_name(&Token::new(
            "labels"
        )));
    }

    #[test]
    fn test_is_semantics_labels_api_path() {
        // P1-2: must parse property paths of the form "semantics:labels:<name>"

        // Valid path should return the instance name
        let path = Path::from_string("/Prim.semantics:labels:taxonomy").unwrap();
        let result = LabelsAPI::is_semantics_labels_api_path(&path);
        assert_eq!(result.as_ref().map(|t| t.as_str()), Some("taxonomy"));

        // Must reject prim paths (not property paths)
        let prim_path = Path::from_string("/Prim").unwrap();
        assert!(LabelsAPI::is_semantics_labels_api_path(&prim_path).is_none());

        // Must reject wrong namespace prefix
        let wrong_path = Path::from_string("/Prim.other:labels:taxonomy").unwrap();
        assert!(LabelsAPI::is_semantics_labels_api_path(&wrong_path).is_none());

        // Must reject paths without instance name
        let short_path = Path::from_string("/Prim.semantics:labels").unwrap();
        assert!(LabelsAPI::is_semantics_labels_api_path(&short_path).is_none());
    }
}
