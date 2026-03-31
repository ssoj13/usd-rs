//! UsdTyped - base for typed (IsA) schemas.

use super::prim::Prim;
use super::schema_base::SchemaBase;
use usd_tf::Token;

// ============================================================================
// Typed
// ============================================================================

/// Base class for typed schemas (IsA schemas).
///
/// Typed schemas define a prim's type. A prim can only have one type,
/// though it may also have API schemas applied.
///
/// The type hierarchy is:
/// ```text
/// UsdSchemaBase
///   └── UsdTyped
///         ├── UsdGeomImageable
///         │     └── UsdGeomXformable
///         │           └── UsdGeomMesh
///         └── ...
/// ```
#[derive(Debug, Clone)]
pub struct Typed {
    /// Base schema.
    inner: SchemaBase,
}

impl Typed {
    /// Creates a Typed schema from a prim.
    pub fn new(prim: Prim) -> Self {
        Self {
            inner: SchemaBase::new(prim),
        }
    }

    /// Creates an invalid Typed schema.
    pub fn invalid() -> Self {
        Self {
            inner: SchemaBase::invalid(),
        }
    }

    /// Returns true if this schema is valid.
    pub fn is_valid(&self) -> bool {
        self.inner.is_valid()
    }

    /// Returns the wrapped prim.
    pub fn prim(&self) -> &Prim {
        self.inner.prim()
    }

    /// Returns the schema type name.
    pub fn schema_type_name() -> Token {
        Token::new("Typed")
    }

    /// Gets the schema base.
    pub fn schema_base(&self) -> &SchemaBase {
        &self.inner
    }

    /// Return a vector of names of all pre-declared attributes for this schema
    /// class and all its ancestor classes.
    ///
    /// Matches C++ `GetSchemaAttributeNames()`.
    pub fn get_schema_attribute_names(include_inherited: bool) -> Vec<Token> {
        // Typed doesn't add any attributes itself, so return empty or inherited
        if include_inherited {
            // Get inherited names from SchemaBase
            SchemaBase::get_schema_attribute_names(true)
        } else {
            Vec::new()
        }
    }
}

impl PartialEq for Typed {
    fn eq(&self, other: &Self) -> bool {
        self.inner == other.inner
    }
}

impl Eq for Typed {}

impl std::hash::Hash for Typed {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.inner.hash(state);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_invalid_typed() {
        let typed = Typed::invalid();
        assert!(!typed.is_valid());
    }

    #[test]
    fn test_schema_type_name() {
        assert_eq!(Typed::schema_type_name().get_text(), "Typed");
    }
}
