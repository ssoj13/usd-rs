//! Generative Procedural schema.
//!
//! Abstract procedural prim that delivers parameters via primvars namespace.
//!
//! # C++ Reference
//!
//! Port of `pxr/usd/usdProc/generativeProcedural.h`

use std::sync::Arc;

use usd_core::{Attribute, Prim, Stage};
use usd_sdf::{Path, ValueTypeRegistry};
use usd_tf::Token;
use usd_vt::Value;

use super::tokens::USD_PROC_TOKENS;

/// Generative procedural prim.
///
/// Represents an abstract generative procedural that delivers input
/// parameters via properties in the "primvars:" namespace.
///
/// # Schema Kind
///
/// This is a concrete typed schema (ConcreteTyped).
///
/// # Attributes
///
/// - `proceduralSystem` - Token identifying the procedural system
#[derive(Debug, Clone)]
pub struct GenerativeProcedural {
    prim: Prim,
}

impl GenerativeProcedural {
    /// The schema type name.
    pub const SCHEMA_TYPE_NAME: &'static str = "GenerativeProcedural";

    /// Construct a GenerativeProcedural on the given prim.
    pub fn new(prim: Prim) -> Self {
        Self { prim }
    }

    /// Construct from another prim.
    pub fn from_prim(prim: &Prim) -> Self {
        Self::new(prim.clone())
    }

    /// Return a GenerativeProcedural holding the prim at `path` on `stage`.
    ///
    /// Matches C++ `Get()` -- no type check performed.
    pub fn get(stage: &Arc<Stage>, path: &Path) -> Option<Self> {
        let prim = stage.get_prim_at_path(path)?;
        Some(Self::new(prim))
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
    // ProceduralSystem Attribute
    // =========================================================================

    /// Get the proceduralSystem attribute.
    ///
    /// Token identifying which system interprets this procedural.
    pub fn get_procedural_system_attr(&self) -> Option<Attribute> {
        self.prim
            .get_attribute(USD_PROC_TOKENS.procedural_system.as_str())
    }

    /// Creates the proceduralSystem attribute.
    ///
    /// Matches C++ `CreateProceduralSystemAttr(VtValue, bool)`.
    pub fn create_procedural_system_attr(
        &self,
        default_value: Option<&Value>,
        _write_sparsely: bool,
    ) -> Attribute {
        if !self.prim.is_valid() {
            return Attribute::invalid();
        }

        let registry = ValueTypeRegistry::instance();
        let token_type = registry.find_type_by_token(&Token::new("token"));

        let attr = self
            .prim
            .create_attribute(
                USD_PROC_TOKENS.procedural_system.as_str(),
                &token_type,
                false,
                // C++ uses SdfVariabilityVarying for proceduralSystem
                Some(usd_core::attribute::Variability::Varying),
            )
            .unwrap_or_else(Attribute::invalid);

        if let Some(val) = default_value {
            attr.set(val.clone(), usd_sdf::TimeCode::default());
        }

        attr
    }

    // =========================================================================
    // Schema attribute names
    // =========================================================================

    /// Returns all pre-declared attributes for this schema class.
    ///
    /// When `include_inherited` is true, includes inherited attrs from
    /// UsdGeomBoundable hierarchy (Boundable < Xformable < Imageable < Typed).
    pub fn get_schema_attribute_names(include_inherited: bool) -> Vec<Token> {
        let local = vec![USD_PROC_TOKENS.procedural_system.clone()];
        if include_inherited {
            // C++ calls UsdGeomBoundable::GetSchemaAttributeNames(true)
            // which returns Imageable + Xformable + Boundable attrs.
            // Hardcoded here to avoid circular dep on usd-geom.
            let mut inherited = vec![
                // Imageable attrs
                Token::new("visibility"),
                Token::new("purpose"),
                Token::new("proxyPrim"),
                // Xformable attrs
                Token::new("xformOpOrder"),
                // Boundable attrs
                Token::new("extent"),
            ];
            inherited.extend(local);
            inherited
        } else {
            local
        }
    }
}

impl From<Prim> for GenerativeProcedural {
    fn from(prim: Prim) -> Self {
        Self::new(prim)
    }
}

impl From<GenerativeProcedural> for Prim {
    fn from(proc: GenerativeProcedural) -> Self {
        proc.prim
    }
}

impl AsRef<Prim> for GenerativeProcedural {
    fn as_ref(&self) -> &Prim {
        &self.prim
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_schema_type_name() {
        assert_eq!(
            GenerativeProcedural::SCHEMA_TYPE_NAME,
            "GenerativeProcedural"
        );
    }

    #[test]
    fn test_schema_attribute_names() {
        let names = GenerativeProcedural::get_schema_attribute_names(false);
        assert!(names.iter().any(|n| n == "proceduralSystem"));
    }
}
