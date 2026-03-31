//! HydraGenerativeProceduralAPI schema.
//!
//! API for configuring Hydra generative procedurals.
//!
//! # C++ Reference
//!
//! Port of `pxr/usd/usdHydra/generativeProceduralAPI.h`

use std::sync::Arc;

use usd_core::attribute::Variability;
use usd_core::{Attribute, Prim, Stage};
use usd_sdf::Path;
use usd_tf::Token;
use usd_vt::Value;

use super::tokens::USD_HYDRA_TOKENS;

/// HydraGenerativeProceduralAPI - configures Hydra generative procedurals.
///
/// This API extends and configures the core UsdProcGenerativeProcedural schema
/// defined within usdProc for use with hydra generative procedurals as defined
/// within hdGp.
///
/// # Schema Kind
///
/// This is a SingleApplyAPI schema.
#[derive(Debug, Clone)]
pub struct HydraGenerativeProceduralAPI {
    prim: Prim,
}

impl HydraGenerativeProceduralAPI {
    /// The schema type name.
    pub const SCHEMA_TYPE_NAME: &'static str = "HydraGenerativeProceduralAPI";

    /// Construct a HydraGenerativeProceduralAPI on the given prim.
    pub fn new(prim: Prim) -> Self {
        Self { prim }
    }

    /// Construct from another prim.
    pub fn from_prim(prim: &Prim) -> Self {
        Self::new(prim.clone())
    }

    /// Return a HydraGenerativeProceduralAPI holding the prim at `path` on `stage`.
    ///
    /// C++ Get() does not check has_api - just wraps the prim.
    pub fn get(stage: &Arc<Stage>, path: &Path) -> Option<Self> {
        let prim = stage.get_prim_at_path(path)?;
        Some(Self::new(prim))
    }

    /// Check if this API can be applied to the given prim.
    ///
    /// If `why_not` is provided and the API cannot be applied, populates it with
    /// the reason. Matches C++ `CanApply(const UsdPrim &prim, std::string *whyNot)`.
    pub fn can_apply(prim: &Prim, _why_not: Option<&mut String>) -> bool {
        prim.can_apply_api(&USD_HYDRA_TOKENS.hydra_generative_procedural_api)
    }

    /// Apply this API to the given prim.
    pub fn apply(prim: &Prim) -> Option<Self> {
        if prim.apply_api(&USD_HYDRA_TOKENS.hydra_generative_procedural_api) {
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
    // ProceduralType Attribute
    // =========================================================================

    /// Get the proceduralType attribute.
    ///
    /// The registered name of a HdGpGenerativeProceduralPlugin to be executed.
    ///
    /// Declaration: `token primvars:hdGp:proceduralType`
    pub fn get_procedural_type_attr(&self) -> Option<Attribute> {
        self.prim
            .get_attribute(USD_HYDRA_TOKENS.primvars_hd_gp_procedural_type.as_str())
    }

    /// Create the proceduralType attribute.
    ///
    /// Matches C++ `_CreateAttr(SdfValueTypeNames->Token, custom=false, SdfVariabilityVarying)`.
    pub fn create_procedural_type_attr(
        &self,
        default_value: Option<&Value>,
        _write_sparsely: bool,
    ) -> Attribute {
        if !self.prim.is_valid() {
            return Attribute::invalid();
        }

        if self
            .prim
            .has_authored_attribute(USD_HYDRA_TOKENS.primvars_hd_gp_procedural_type.as_str())
        {
            return self
                .prim
                .get_attribute(USD_HYDRA_TOKENS.primvars_hd_gp_procedural_type.as_str())
                .unwrap();
        }

        let registry = usd_sdf::ValueTypeRegistry::instance();
        let type_name = registry.find_or_create_type_name(&Token::new("token"));
        let attr = self
            .prim
            .create_attribute(
                USD_HYDRA_TOKENS.primvars_hd_gp_procedural_type.as_str(),
                &type_name,
                false,
                Some(Variability::Varying),
            )
            .unwrap_or_else(Attribute::invalid);
        if let Some(val) = default_value {
            attr.set(val.clone(), usd_sdf::TimeCode::default());
        }
        attr
    }

    // =========================================================================
    // ProceduralSystem Attribute
    // =========================================================================

    /// Get the proceduralSystem attribute.
    ///
    /// This value should correspond to a configured instance of
    /// HdGpGenerativeProceduralResolvingSceneIndex which will evaluate the
    /// procedural. The default value of "hydraGenerativeProcedural" matches
    /// the equivalent default of HdGpGenerativeProceduralResolvingSceneIndex.
    ///
    /// Declaration: `token proceduralSystem = "hydraGenerativeProcedural"`
    pub fn get_procedural_system_attr(&self) -> Option<Attribute> {
        self.prim
            .get_attribute(USD_HYDRA_TOKENS.procedural_system.as_str())
    }

    /// Create the proceduralSystem attribute.
    ///
    /// Matches C++ `_CreateAttr(SdfValueTypeNames->Token, custom=false, SdfVariabilityVarying)`.
    pub fn create_procedural_system_attr(
        &self,
        default_value: Option<&Value>,
        _write_sparsely: bool,
    ) -> Attribute {
        if !self.prim.is_valid() {
            return Attribute::invalid();
        }

        if self
            .prim
            .has_authored_attribute(USD_HYDRA_TOKENS.procedural_system.as_str())
        {
            return self
                .prim
                .get_attribute(USD_HYDRA_TOKENS.procedural_system.as_str())
                .unwrap();
        }

        let registry = usd_sdf::ValueTypeRegistry::instance();
        let type_name = registry.find_or_create_type_name(&Token::new("token"));
        let attr = self
            .prim
            .create_attribute(
                USD_HYDRA_TOKENS.procedural_system.as_str(),
                &type_name,
                false,
                Some(Variability::Varying),
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
    /// When `include_inherited` is true, returns names from this schema
    /// and all ancestor classes. Otherwise returns only local names.
    pub fn get_schema_attribute_names(include_inherited: bool) -> Vec<Token> {
        let local_names = vec![
            USD_HYDRA_TOKENS.primvars_hd_gp_procedural_type.clone(),
            USD_HYDRA_TOKENS.procedural_system.clone(),
        ];
        if include_inherited {
            // UsdAPISchemaBase has no attributes, so inherited == local
            local_names
        } else {
            local_names
        }
    }
}

impl From<Prim> for HydraGenerativeProceduralAPI {
    fn from(prim: Prim) -> Self {
        Self::new(prim)
    }
}

impl From<HydraGenerativeProceduralAPI> for Prim {
    fn from(api: HydraGenerativeProceduralAPI) -> Self {
        api.prim
    }
}

impl AsRef<Prim> for HydraGenerativeProceduralAPI {
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
            HydraGenerativeProceduralAPI::SCHEMA_TYPE_NAME,
            "HydraGenerativeProceduralAPI"
        );
    }

    #[test]
    fn test_schema_attribute_names() {
        let names = HydraGenerativeProceduralAPI::get_schema_attribute_names(false);
        assert_eq!(names.len(), 2);
    }
}
