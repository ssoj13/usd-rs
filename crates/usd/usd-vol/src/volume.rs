//! Volume schema.
//!
//! A renderable volume primitive. A volume is made up of any number of
//! FieldBase primitives bound together via namespaced relationships.
//!
//! # C++ Reference
//!
//! Port of `pxr/usd/usdVol/volume.h`

use std::collections::HashMap;
use std::sync::Arc;

use usd_core::{Prim, Relationship, Stage};
use usd_sdf::Path;
use usd_tf::Token;

use super::tokens::USD_VOL_TOKENS;

/// Type alias for field name to path mapping.
pub type FieldMap = HashMap<Token, Path>;

/// Renderable volume primitive.
///
/// A volume is made up of any number of FieldBase primitives bound together
/// via namespaced relationships (field:*). The relationship name maps to
/// shader input parameters.
///
/// # Field Relationships
///
/// Each field relationship uses the "field:" namespace prefix. The name
/// after the prefix maps to the shader parameter name.
///
/// # Schema Kind
///
/// This is a concrete typed schema (ConcreteTyped).
///
/// # Inheritance
///
/// Inherits from UsdGeomGprim (not directly modeled here).
#[derive(Debug, Clone)]
pub struct Volume {
    prim: Prim,
}

impl Volume {
    /// The schema type name.
    pub const SCHEMA_TYPE_NAME: &'static str = "Volume";

    /// The namespace prefix for field relationships.
    pub const FIELD_PREFIX: &'static str = "field:";

    /// Construct a Volume on the given prim.
    pub fn new(prim: Prim) -> Self {
        Self { prim }
    }

    /// Construct from another prim.
    pub fn from_prim(prim: &Prim) -> Self {
        Self::new(prim.clone())
    }

    /// Return a Volume holding the prim at `path` on `stage`.
    pub fn get(stage: &Arc<Stage>, path: &Path) -> Option<Self> {
        let prim = stage.get_prim_at_path(path)?;
        if prim.is_a(&USD_VOL_TOKENS.volume) {
            Some(Self::new(prim))
        } else {
            None
        }
    }

    /// Attempt to ensure a prim adhering to this schema at `path` is defined.
    pub fn define(stage: &Arc<Stage>, path: &Path) -> Option<Self> {
        let prim = stage
            .define_prim(path.as_str(), Self::SCHEMA_TYPE_NAME)
            .ok()?;
        Some(Self::new(prim))
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
    // Field Attachment and Inspection
    // =========================================================================

    /// Return a map of field relationship names to their target paths.
    ///
    /// The keys have the field namespace prefix stripped.
    /// Uses forwarded targets to resolve relationship forwarding.
    pub fn get_field_paths(&self) -> FieldMap {
        let mut result = FieldMap::new();

        // Get all properties in the field namespace
        let field_token = Token::new(Self::FIELD_PREFIX.trim_end_matches(':'));
        let props = self.prim.get_properties_in_namespace(&field_token);

        for prop in props {
            if let Some(rel) = prop.as_relationship() {
                // Use forwarded targets to resolve relationship forwarding
                let targets = rel.get_forwarded_targets();

                // Only include relationships with exactly one prim path target
                if targets.len() == 1 && targets[0].is_prim_path() {
                    // Strip the namespace prefix from the relationship name
                    let full_name = rel.name();
                    if let Some(stripped) = full_name.as_str().strip_prefix(Self::FIELD_PREFIX) {
                        result.insert(Token::new(stripped), targets[0].clone());
                    }
                }
            }
        }

        result
    }

    /// Check if a field relationship with the given name exists.
    ///
    /// The name will be automatically prefixed with "field:" if not present.
    pub fn has_field_relationship(&self, name: &Token) -> bool {
        let namespaced = Self::make_namespaced(name);
        self.prim.get_relationship(namespaced.as_str()).is_some()
    }

    /// Get the path to the field prim for the given field name.
    ///
    /// Returns None if no relationship exists or it has no target.
    /// Uses forwarded targets to resolve relationship forwarding.
    pub fn get_field_path(&self, name: &Token) -> Option<Path> {
        let namespaced = Self::make_namespaced(name);
        let rel = self.prim.get_relationship(namespaced.as_str())?;

        // Use forwarded targets to resolve relationship forwarding
        let targets = rel.get_forwarded_targets();

        // Return the first target if it's a prim path
        if targets.len() == 1 && targets[0].is_prim_path() {
            Some(targets[0].clone())
        } else {
            None
        }
    }

    /// Create a relationship targeting the specified field.
    ///
    /// Replaces any existing relationship with the same name.
    ///
    /// # Arguments
    ///
    /// * `name` - Field name (will be prefixed with "field:" if needed)
    /// * `field_path` - Path to the field prim or forwarding relationship
    ///
    /// # Returns
    ///
    /// Returns false if the path is invalid or relationship creation fails.
    pub fn create_field_relationship(&self, name: &Token, field_path: &Path) -> bool {
        // Validate path: must be a prim path or prim property path (for forwarding)
        if !field_path.is_prim_path() && !field_path.is_prim_property_path() {
            return false;
        }

        let namespaced = Self::make_namespaced(name);

        // Create as custom relationship (custom=true)
        if let Some(rel) = self.prim.create_relationship(namespaced.as_str(), true) {
            rel.set_targets(&[field_path.clone()])
        } else {
            false
        }
    }

    /// Block an existing field relationship.
    ///
    /// The relationship will not appear in `get_field_paths()` after blocking.
    ///
    /// Returns true if the relationship existed and was blocked.
    pub fn block_field_relationship(&self, name: &Token) -> bool {
        let namespaced = Self::make_namespaced(name);
        if let Some(rel) = self.prim.get_relationship(namespaced.as_str()) {
            // Block targets — authors explicit empty list that blocks weaker opinions
            rel.block_targets();
            true
        } else {
            false
        }
    }

    /// Make a field name namespaced with the "field:" prefix.
    fn make_namespaced(name: &Token) -> Token {
        let text = name.as_str();
        if text.starts_with(Self::FIELD_PREFIX) {
            name.clone()
        } else {
            Token::new(&format!("{}{}", Self::FIELD_PREFIX, text))
        }
    }

    // =========================================================================
    // Schema attribute names
    // =========================================================================

    /// Returns all pre-declared attributes for this schema class.
    ///
    /// Volume has no custom attributes beyond those from Gprim.
    pub fn get_schema_attribute_names(_include_inherited: bool) -> Vec<Token> {
        Vec::new()
    }
}

impl Volume {
    /// Get a specific field relationship by name.
    pub fn get_field_rel(&self, name: &Token) -> Option<Relationship> {
        let namespaced = Self::make_namespaced(name);
        self.prim.get_relationship(namespaced.as_str())
    }
}

impl From<Prim> for Volume {
    fn from(prim: Prim) -> Self {
        Self::new(prim)
    }
}

impl From<Volume> for Prim {
    fn from(volume: Volume) -> Self {
        volume.prim
    }
}

impl AsRef<Prim> for Volume {
    fn as_ref(&self) -> &Prim {
        &self.prim
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_schema_type_name() {
        assert_eq!(Volume::SCHEMA_TYPE_NAME, "Volume");
    }

    #[test]
    fn test_field_prefix() {
        assert_eq!(Volume::FIELD_PREFIX, "field:");
    }

    #[test]
    fn test_make_namespaced() {
        // Already namespaced
        let namespaced = Token::new("field:density");
        assert_eq!(
            Volume::make_namespaced(&namespaced).as_str(),
            "field:density"
        );

        // Not namespaced
        let plain = Token::new("density");
        assert_eq!(Volume::make_namespaced(&plain).as_str(), "field:density");
    }

    #[test]
    fn test_schema_attribute_names() {
        let names = Volume::get_schema_attribute_names(false);
        assert!(names.is_empty());
    }
}
