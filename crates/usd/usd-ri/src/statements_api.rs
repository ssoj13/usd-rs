//! StatementsAPI schema.
//!
//! Container namespace schema for all RenderMan statements.
//!
//! # C++ Reference
//!
//! Port of `pxr/usd/usdRi/statementsAPI.h` and `statementsAPI.cpp`

use std::any::TypeId;
use std::sync::{Arc, OnceLock};

use usd_core::{Attribute, Prim, Stage};
use usd_sdf::Path;
use usd_sdf::TimeCode;
use usd_tf::Token;
use usd_vt::Value;

use super::tokens::USD_RI_TOKENS;
use super::type_utils::get_usd_type;

/// Primvar-style attribute namespace (new encoding).
const PRIMVAR_ATTR_NAMESPACE: &str = "primvars:ri:attributes:";

/// Old-style attribute namespace.
const FULL_ATTR_NAMESPACE: &str = "ri:attributes:";

/// Returns true if old-style ri:attributes: encoding should be read.
/// Controlled by USDRI_STATEMENTS_READ_OLD_ATTR_ENCODING env var (default: true).
/// Matches C++ TF_DEFINE_ENV_SETTING(USDRI_STATEMENTS_READ_OLD_ATTR_ENCODING, true, ...).
fn read_old_attr_encoding() -> bool {
    static VALUE: OnceLock<bool> = OnceLock::new();
    *VALUE.get_or_init(|| {
        std::env::var("USDRI_STATEMENTS_READ_OLD_ATTR_ENCODING")
            .map(|v| !matches!(v.as_str(), "0" | "false" | "False" | "FALSE"))
            .unwrap_or(true)
    })
}

/// StatementsAPI - container for RenderMan statements.
///
/// This API schema provides functionality for authoring and reading
/// RenderMan-specific attributes on USD prims.
///
/// # Attribute Namespacing
///
/// Ri attributes are stored in the "primvars:ri:attributes:" namespace
/// (new encoding) or the "ri:attributes:" namespace (old encoding).
///
/// # Schema Kind
///
/// This is a SingleApplyAPI schema.
#[derive(Debug, Clone)]
pub struct StatementsAPI {
    prim: Prim,
}

impl StatementsAPI {
    /// The schema type name.
    pub const SCHEMA_TYPE_NAME: &'static str = "StatementsAPI";

    /// Ri attributes namespace prefix (old encoding for backward compat).
    pub const RI_ATTRIBUTES_PREFIX: &'static str = FULL_ATTR_NAMESPACE;

    /// Primvar-based namespace prefix (new encoding).
    pub const PRIMVAR_ATTRIBUTES_PREFIX: &'static str = PRIMVAR_ATTR_NAMESPACE;

    /// Construct a StatementsAPI on the given prim.
    pub fn new(prim: Prim) -> Self {
        Self { prim }
    }

    /// Construct from another prim.
    pub fn from_prim(prim: &Prim) -> Self {
        Self::new(prim.clone())
    }

    /// Return a StatementsAPI holding the prim at `path` on `stage`.
    pub fn get(stage: &Arc<Stage>, path: &Path) -> Option<Self> {
        let prim = stage.get_prim_at_path(path)?;
        if prim.has_api(&USD_RI_TOKENS.statements_api) {
            Some(Self::new(prim))
        } else {
            None
        }
    }

    /// Check if this API can be applied to the given prim.
    pub fn can_apply(prim: &Prim) -> bool {
        prim.can_apply_api(&USD_RI_TOKENS.statements_api)
    }

    /// Apply this API to the given prim.
    pub fn apply(prim: &Prim) -> Option<Self> {
        if prim.apply_api(&USD_RI_TOKENS.statements_api) {
            Some(Self::new(prim.clone()))
        } else {
            None
        }
    }

    /// Returns true if this schema is valid.
    pub fn is_valid(&self) -> bool {
        self.prim.is_valid()
    }

    /// Get the wrapped prim.
    pub fn get_prim(&self) -> &Prim {
        &self.prim
    }

    // =========================================================================
    // Ri Attribute Creation
    // =========================================================================

    /// Build the full ri:attributes:namespace:name token.
    fn make_ri_attr_namespace(namespace: &str, attr_name: &str) -> Token {
        Token::new(&format!(
            "{}{}:{}",
            FULL_ATTR_NAMESPACE, namespace, attr_name
        ))
    }

    /// Create a rib attribute on the prim using a RenderMan type string.
    ///
    /// A rib attribute consists of a namespace and name. For example,
    /// the namespace "cull" may define attributes "backfacing" and "hidden".
    /// User-defined attributes belong to the namespace "user".
    ///
    /// Creates the attribute via PrimvarsAPI-style encoding (primvar namespace).
    ///
    /// # Arguments
    ///
    /// * `name` - The attribute name
    /// * `ri_type` - RenderMan type definition (e.g. "color", "float[3]")
    /// * `namespace` - The namespace (default: "user")
    pub fn create_ri_attribute(
        &self,
        name: &Token,
        ri_type: &str,
        namespace: &str,
    ) -> Option<Attribute> {
        let full_name = Self::make_ri_attr_namespace(namespace, name.as_str());
        let usd_type = get_usd_type(ri_type);
        // Create via primvar-style: C++ uses UsdGeomPrimvarsAPI(GetPrim()).CreatePrimvar()
        let primvar_name = format!("primvars:{}", full_name.as_str());
        self.prim
            .create_attribute(&primvar_name, &usd_type, false, None)
    }

    /// Create a rib attribute on the prim using a Rust TypeId.
    ///
    /// This is the equivalent of the C++ `CreateRiAttribute(name, TfType, namespace)`
    /// overload. Instead of TfType, Rust uses `std::any::TypeId`.
    ///
    /// # Arguments
    ///
    /// * `name` - The attribute name
    /// * `type_id` - Rust TypeId for the value type
    /// * `namespace` - The namespace (default: "user")
    pub fn create_ri_attribute_typed(
        &self,
        name: &Token,
        type_id: TypeId,
        namespace: &str,
    ) -> Option<Attribute> {
        let full_name = Self::make_ri_attr_namespace(namespace, name.as_str());
        let registry = usd_sdf::value_type_registry::ValueTypeRegistry::instance();
        let usd_type = registry.find_type_by_type_id(type_id, None);
        let primvar_name = format!("primvars:{}", full_name.as_str());
        self.prim
            .create_attribute(&primvar_name, &usd_type, false, None)
    }

    /// Get a UsdAttribute representing the Ri attribute.
    ///
    /// Checks primvar encoding first, then falls back to old-style encoding
    /// only if USDRI_STATEMENTS_READ_OLD_ATTR_ENCODING is enabled (default: true).
    /// Matches C++ `UsdRiStatementsAPI::GetRiAttribute()`.
    pub fn get_ri_attribute(&self, name: &Token, namespace: &str) -> Option<Attribute> {
        let full_name = Self::make_ri_attr_namespace(namespace, name.as_str());

        // Try primvar encoding first: "primvars:ri:attributes:ns:name"
        let primvar_name = format!("primvars:{}", full_name.as_str());
        if let Some(attr) = self.prim.get_attribute(&primvar_name) {
            return Some(attr);
        }

        // Fall back to old-style encoding only when env var is set
        if read_old_attr_encoding() {
            return self.prim.get_attribute(full_name.as_str());
        }

        None
    }

    /// Return all rib attributes on this prim.
    ///
    /// Enumerates primvar-encoded attributes first, then optionally
    /// includes old-style encoded attributes (deduplicating) when
    /// USDRI_STATEMENTS_READ_OLD_ATTR_ENCODING is enabled.
    ///
    /// Matches C++ `UsdRiStatementsAPI::GetRiAttributes()`.
    pub fn get_ri_attributes(&self, namespace: &str) -> Vec<Attribute> {
        let mut valid_attrs: Vec<Attribute> = Vec::new();

        // Collect primvar-encoded ri attributes
        let primvar_prefix = if namespace.is_empty() {
            PRIMVAR_ATTR_NAMESPACE.to_string()
        } else {
            format!("{}{}:", PRIMVAR_ATTR_NAMESPACE, namespace)
        };

        // First pass: primvar-encoded attributes
        for attr_name in self.prim.get_attribute_names() {
            let name_str = attr_name.as_str();
            if name_str.starts_with(&primvar_prefix) {
                if let Some(attr) = self.prim.get_attribute(name_str) {
                    valid_attrs.push(attr);
                }
            }
        }

        // Second pass: old-style encoded attributes (only if env var enabled)
        if read_old_attr_encoding() {
            let old_prefix = if namespace.is_empty() {
                FULL_ATTR_NAMESPACE.to_string()
            } else {
                format!("{}{}:", FULL_ATTR_NAMESPACE, namespace)
            };

            let num_primvar_attrs = valid_attrs.len();
            for attr_name in self.prim.get_attribute_names() {
                let name_str = attr_name.as_str();
                if !name_str.starts_with(&old_prefix)
                    || name_str.starts_with(PRIMVAR_ATTR_NAMESPACE)
                {
                    continue;
                }

                // Check for namespace filter
                if !namespace.is_empty() {
                    let stripped = &name_str[FULL_ATTR_NAMESPACE.len()..];
                    let parts: Vec<&str> = stripped.split(':').collect();
                    if parts.is_empty() || parts[0] != namespace {
                        continue;
                    }
                }

                // Dedup: skip if same ri attribute exists as primvar.
                // C++ also checks IsAuthored() to let authored old-style win
                // over non-authored primvars, but Attribute::is_authored()
                // is not yet available in our Rust API.
                let mut found_as_primvar = false;
                for i in 0..num_primvar_attrs {
                    let primvar_name = valid_attrs[i].name();
                    let primvar_str = primvar_name.as_str();
                    if let Some(after_colon) = primvar_str.find(':') {
                        // Compare everything after first colon with old-style name
                        if &primvar_str[after_colon + 1..] == name_str {
                            found_as_primvar = true;
                            break;
                        }
                    }
                }

                if !found_as_primvar {
                    if let Some(attr) = self.prim.get_attribute(name_str) {
                        valid_attrs.push(attr);
                    }
                }
            }
        }

        valid_attrs
    }

    /// Return the base, most-specific name of the rib attribute.
    ///
    /// For example, the name of "ri:attributes:cull:backfacing" is "backfacing".
    /// Also handles "primvars:ri:attributes:cull:backfacing" -> "backfacing".
    pub fn get_ri_attribute_name(attr: &Attribute) -> Token {
        let name = attr.name();
        let name_str = name.as_str();

        // Get the last component after the final colon
        if let Some(pos) = name_str.rfind(':') {
            Token::new(&name_str[pos + 1..])
        } else {
            name
        }
    }

    /// Return the containing namespace of the rib attribute (e.g. "user").
    ///
    /// Handles primvar encoding always, and old-style encoding only when
    /// USDRI_STATEMENTS_READ_OLD_ATTR_ENCODING is enabled.
    /// Matches C++ `UsdRiStatementsAPI::GetRiAttributeNameSpace()`.
    pub fn get_ri_attribute_namespace(attr: &Attribute) -> Token {
        let name = attr.name();
        let name_str = name.as_str();

        // Parse primvar encoding: "primvars:ri:attributes:$(NS_1):...:$(NS_N):$(NAME)"
        if let Some(stripped) = name_str.strip_prefix(PRIMVAR_ATTR_NAMESPACE) {
            let parts: Vec<&str> = stripped.split(':').collect();
            if parts.len() >= 2 {
                // Namespace is everything except the last component
                let ns_parts = &parts[..parts.len() - 1];
                return Token::new(&ns_parts.join(":"));
            }
            return Token::default();
        }

        // Optionally parse old-style encoding: "ri:attributes:$(NS_1):...:$(NS_N):$(NAME)"
        if read_old_attr_encoding() {
            if let Some(stripped) = name_str.strip_prefix(FULL_ATTR_NAMESPACE) {
                let parts: Vec<&str> = stripped.split(':').collect();
                if parts.len() >= 2 {
                    let ns_parts = &parts[..parts.len() - 1];
                    return Token::new(&ns_parts.join(":"));
                }
            }
        }

        Token::default()
    }

    /// Return true if the property is in the "ri:attributes" namespace.
    ///
    /// Accepts primvar encoding always, and old-style encoding only when
    /// USDRI_STATEMENTS_READ_OLD_ATTR_ENCODING is enabled.
    /// Matches C++ `UsdRiStatementsAPI::IsRiAttribute()`.
    pub fn is_ri_attribute(attr: &Attribute) -> bool {
        let name = attr.name();
        let name_str = name.as_str();
        // Accept primvar encoding
        if name_str.starts_with(PRIMVAR_ATTR_NAMESPACE) {
            return true;
        }
        // Optionally accept old-style attribute encoding
        if name_str.starts_with(FULL_ATTR_NAMESPACE) && read_old_attr_encoding() {
            return true;
        }
        false
    }

    /// Returns the given attribute name prefixed with the full Ri attribute
    /// namespace, creating a name suitable for an RiAttribute UsdProperty.
    ///
    /// This handles conversion of common separator characters used in
    /// other packages, such as periods and underscores.
    ///
    /// Output format: "primvars:ri:attributes:namespace:name"
    ///
    /// Rules (matching C++):
    /// - If already a properly constructed RiAttribute property name, return unchanged
    /// - If contains colons, first token is namespace, rest joined by underscores
    /// - If contains periods, first token is namespace, rest joined by underscores
    /// - If contains underscores, first token is namespace, rest is name
    /// - Otherwise, default to "user" namespace
    pub fn make_ri_attribute_property_name(attr_name: &str) -> String {
        // Tokenize by colon first
        let colon_parts: Vec<&str> = attr_name.split(':').collect();

        // Already a fully-encoded primvar name
        if colon_parts.len() == 5 && attr_name.starts_with(PRIMVAR_ATTR_NAMESPACE) {
            return attr_name.to_string();
        }
        // Already a fully-encoded old-style name
        if colon_parts.len() == 4 && attr_name.starts_with(FULL_ATTR_NAMESPACE) {
            return attr_name.to_string();
        }

        // Attempt to parse namespaces in different forms
        let mut names: Vec<String> = colon_parts.iter().map(|s| s.to_string()).collect();

        if names.len() == 1 {
            // Try period separator
            names = attr_name.split('.').map(|s| s.to_string()).collect();
        }
        if names.len() == 1 {
            // Try underscore separator
            names = attr_name.split('_').map(|s| s.to_string()).collect();
        }

        // Fallback to user namespace if no separator found
        if names.len() == 1 {
            names.insert(0, "user".to_string());
        }

        // Build: "primvars:ri:attributes:namespace:name"
        // If more than 2 tokens, join all but first with underscore
        let namespace = &names[0];
        let name = if names.len() > 2 {
            names[1..].join("_")
        } else {
            names[1].clone()
        };

        let full_name = format!("{}{}:{}", PRIMVAR_ATTR_NAMESPACE, namespace, name);

        // Validate as namespaced identifier
        if Path::is_valid_namespaced_identifier(&full_name) {
            full_name
        } else {
            String::new()
        }
    }

    // =========================================================================
    // Coordinate System API (Deprecated)
    // =========================================================================

    /// Sets the "ri:coordinateSystem" attribute.
    ///
    /// Creates the attribute if needed, sets the value, and walks up to
    /// parent leaf model to add relationship targets.
    ///
    /// # Deprecation
    ///
    /// Use UsdShadeCoordSysAPI instead.
    pub fn set_coordinate_system(&self, coord_sys_name: &str) -> bool {
        // Create the attribute with String type
        let string_type =
            usd_sdf::value_type_registry::ValueTypeRegistry::instance().find_type("string");
        let attr = match self.prim.create_attribute(
            USD_RI_TOKENS.ri_coordinate_system.as_str(),
            &string_type,
            false,
            None,
        ) {
            Some(a) => a,
            None => return false,
        };

        // Set the value
        if !attr.set(Value::from(coord_sys_name.to_string()), TimeCode::default()) {
            return false;
        }

        // Walk up to find the nearest leaf model (IsModel && !IsGroup), starting
        // from the prim itself (matching C++ which starts currPrim = GetPrim()).
        let self_path = self.prim.path().clone();
        let mut curr = self.prim.clone();
        while curr.is_valid() && curr.path() != &Path::absolute_root() {
            if curr.is_model() && !curr.is_group() {
                if let Some(rel) = curr
                    .create_relationship(USD_RI_TOKENS.ri_model_coordinate_systems.as_str(), false)
                {
                    rel.add_target(&self_path);
                }
                break;
            }
            curr = curr.parent();
        }

        true
    }

    /// Returns the value in "ri:coordinateSystem" if it exists.
    ///
    /// # Deprecation
    ///
    /// Use UsdShadeCoordSysAPI instead.
    pub fn get_coordinate_system(&self) -> Option<String> {
        let attr = self
            .prim
            .get_attribute(USD_RI_TOKENS.ri_coordinate_system.as_str())?;
        let val = attr.get(TimeCode::default())?;
        val.downcast_clone::<String>()
    }

    /// Returns true if the prim has a ri:coordinateSystem opinion.
    ///
    /// Checks that the attribute exists AND has a value (matches C++ attr.Get()).
    ///
    /// # Deprecation
    ///
    /// Use UsdShadeCoordSysAPI instead.
    pub fn has_coordinate_system(&self) -> bool {
        if let Some(attr) = self
            .prim
            .get_attribute(USD_RI_TOKENS.ri_coordinate_system.as_str())
        {
            attr.get(TimeCode::default()).is_some()
        } else {
            false
        }
    }

    /// Sets the "ri:scopedCoordinateSystem" attribute.
    ///
    /// Creates the attribute if needed, sets the value, and walks up to
    /// parent leaf model to add relationship targets.
    ///
    /// # Deprecation
    ///
    /// Use UsdShadeCoordSysAPI instead.
    pub fn set_scoped_coordinate_system(&self, coord_sys_name: &str) -> bool {
        let string_type =
            usd_sdf::value_type_registry::ValueTypeRegistry::instance().find_type("string");
        let attr = match self.prim.create_attribute(
            USD_RI_TOKENS.ri_scoped_coordinate_system.as_str(),
            &string_type,
            false,
            None,
        ) {
            Some(a) => a,
            None => return false,
        };

        if !attr.set(Value::from(coord_sys_name.to_string()), TimeCode::default()) {
            return false;
        }

        // Walk up to find the nearest leaf model, starting from the prim itself
        // (matching C++ which starts currPrim = GetPrim()).
        let self_path = self.prim.path().clone();
        let mut curr = self.prim.clone();
        while curr.is_valid() {
            if curr.is_model() && !curr.is_group() && curr.path() != &Path::absolute_root() {
                if let Some(rel) = curr.create_relationship(
                    USD_RI_TOKENS.ri_model_scoped_coordinate_systems.as_str(),
                    false,
                ) {
                    rel.add_target(&self_path);
                }
                break;
            }
            curr = curr.parent();
        }

        true
    }

    /// Returns the value in "ri:scopedCoordinateSystem" if it exists.
    ///
    /// # Deprecation
    ///
    /// Use UsdShadeCoordSysAPI instead.
    pub fn get_scoped_coordinate_system(&self) -> Option<String> {
        let attr = self
            .prim
            .get_attribute(USD_RI_TOKENS.ri_scoped_coordinate_system.as_str())?;
        let val = attr.get(TimeCode::default())?;
        val.downcast_clone::<String>()
    }

    /// Returns true if the prim has a ri:scopedCoordinateSystem opinion.
    ///
    /// # Deprecation
    ///
    /// Use UsdShadeCoordSysAPI instead.
    pub fn has_scoped_coordinate_system(&self) -> bool {
        if let Some(attr) = self
            .prim
            .get_attribute(USD_RI_TOKENS.ri_scoped_coordinate_system.as_str())
        {
            attr.get(TimeCode::default()).is_some()
        } else {
            false
        }
    }

    /// Get the model coordinate systems relationship targets.
    ///
    /// Only queries the relationship if the prim IsModel().
    /// Uses GetForwardedTargets (resolves forwarded targets).
    ///
    /// # Deprecation
    ///
    /// Use UsdShadeCoordSysAPI instead.
    pub fn get_model_coordinate_systems(&self) -> Option<Vec<Path>> {
        if !self.prim.is_model() {
            return Some(Vec::new());
        }
        let rel = self
            .prim
            .get_relationship(USD_RI_TOKENS.ri_model_coordinate_systems.as_str())?;
        Some(rel.get_forwarded_targets())
    }

    /// Get the model scoped coordinate systems relationship targets.
    ///
    /// Only queries the relationship if the prim IsModel().
    /// Uses GetForwardedTargets (resolves forwarded targets).
    ///
    /// # Deprecation
    ///
    /// Use UsdShadeCoordSysAPI instead.
    pub fn get_model_scoped_coordinate_systems(&self) -> Option<Vec<Path>> {
        if !self.prim.is_model() {
            return Some(Vec::new());
        }
        let rel = self
            .prim
            .get_relationship(USD_RI_TOKENS.ri_model_scoped_coordinate_systems.as_str())?;
        Some(rel.get_forwarded_targets())
    }

    // =========================================================================
    // Schema attribute names
    // =========================================================================

    /// Returns all pre-declared attributes for this schema class.
    pub fn get_schema_attribute_names(_include_inherited: bool) -> Vec<Token> {
        // StatementsAPI has no fixed attributes
        Vec::new()
    }
}

impl From<Prim> for StatementsAPI {
    fn from(prim: Prim) -> Self {
        Self::new(prim)
    }
}

impl From<StatementsAPI> for Prim {
    fn from(api: StatementsAPI) -> Self {
        api.prim
    }
}

impl AsRef<Prim> for StatementsAPI {
    fn as_ref(&self) -> &Prim {
        &self.prim
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_schema_type_name() {
        assert_eq!(StatementsAPI::SCHEMA_TYPE_NAME, "StatementsAPI");
    }

    #[test]
    fn test_make_ri_attribute_property_name() {
        // Already namespaced (primvar encoding)
        let name =
            StatementsAPI::make_ri_attribute_property_name("primvars:ri:attributes:user:foo");
        assert_eq!(name, "primvars:ri:attributes:user:foo");

        // Colon separator -> "primvars:ri:attributes:cull:backfacing"
        let name = StatementsAPI::make_ri_attribute_property_name("cull:backfacing");
        assert_eq!(name, "primvars:ri:attributes:cull:backfacing");

        // Period separator
        let name = StatementsAPI::make_ri_attribute_property_name("user.myattr");
        assert_eq!(name, "primvars:ri:attributes:user:myattr");

        // Underscore separator
        let name = StatementsAPI::make_ri_attribute_property_name("user_myattr");
        assert_eq!(name, "primvars:ri:attributes:user:myattr");

        // No separator - default to user namespace
        let name = StatementsAPI::make_ri_attribute_property_name("myattr");
        assert_eq!(name, "primvars:ri:attributes:user:myattr");

        // Multiple colons: first is namespace, rest joined by underscore
        let name = StatementsAPI::make_ri_attribute_property_name("cull:back:facing");
        assert_eq!(name, "primvars:ri:attributes:cull:back_facing");
    }
}
