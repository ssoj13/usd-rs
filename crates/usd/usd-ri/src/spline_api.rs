//! RiSplineAPI schema.
//!
//! A general purpose API schema for describing named splines.
//!
//! # Deprecation Notice
//!
//! This API schema is deprecated and will be removed in a future release.
//!
//! # C++ Reference
//!
//! Port of `pxr/usd/usdRi/splineAPI.h`

use std::sync::Arc;

use super::tokens::USD_RI_TOKENS;
use usd_core::attribute::Variability;
use usd_core::{Attribute, Prim, Stage};
use usd_sdf::{Path, TimeCode, ValueTypeName};
use usd_tf::Token;

/// RiSplineAPI - describes a named spline stored as attributes on a prim.
///
/// It is an add-on schema that can be applied many times to a prim with
/// different spline names. All the attributes authored by the schema
/// are namespaced under "$NAME:spline:", with the name of the
/// spline providing a namespace for the attributes.
///
/// The spline describes a 2D piecewise cubic curve with a position and
/// value for each knot.
///
/// # Supported Basis Types
///
/// - linear (USD_RI_TOKENS.linear)
/// - bspline (USD_RI_TOKENS.bspline)
/// - Catmull-Rom (USD_RI_TOKENS.catmull_rom)
///
/// # Schema Kind
///
/// This is a SingleApplyAPI schema.
///
/// # Deprecation
///
/// This API schema is deprecated.
#[derive(Debug, Clone)]
pub struct RiSplineAPI {
    prim: Prim,
    spline_name: Token,
    values_type_name: ValueTypeName,
    duplicate_bspline_endpoints: bool,
}

impl RiSplineAPI {
    /// The schema type name.
    pub const SCHEMA_TYPE_NAME: &'static str = "RiSplineAPI";

    /// Construct a RiSplineAPI on the given prim.
    pub fn new(prim: Prim) -> Self {
        Self {
            prim,
            spline_name: Token::default(),
            values_type_name: ValueTypeName::default(),
            duplicate_bspline_endpoints: false,
        }
    }

    /// Construct with spline name and value type.
    pub fn with_spline_name(
        prim: Prim,
        spline_name: Token,
        values_type_name: ValueTypeName,
        duplicate_bspline_endpoints: bool,
    ) -> Self {
        Self {
            prim,
            spline_name,
            values_type_name,
            duplicate_bspline_endpoints,
        }
    }

    /// Construct from another prim.
    pub fn from_prim(prim: &Prim) -> Self {
        Self::new(prim.clone())
    }

    /// Return a RiSplineAPI holding the prim at `path` on `stage`.
    pub fn get(stage: &Arc<Stage>, path: &Path) -> Option<Self> {
        let prim = stage.get_prim_at_path(path)?;
        if prim.has_api(&USD_RI_TOKENS.ri_spline_api) {
            Some(Self::new(prim))
        } else {
            None
        }
    }

    /// Check if this API can be applied to the given prim.
    pub fn can_apply(prim: &Prim) -> bool {
        prim.can_apply_api(&USD_RI_TOKENS.ri_spline_api)
    }

    /// Apply this API to the given prim.
    pub fn apply(prim: &Prim) -> Option<Self> {
        if prim.apply_api(&USD_RI_TOKENS.ri_spline_api) {
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

    /// Returns true if this API duplicates BSpline endpoints.
    ///
    /// Duplicating the endpoints ensures that the spline reaches
    /// those points at either end of the parameter range.
    pub fn does_duplicate_bspline_endpoints(&self) -> bool {
        self.duplicate_bspline_endpoints
    }

    /// Returns the intended typename of the values attribute.
    pub fn get_values_type_name(&self) -> &ValueTypeName {
        &self.values_type_name
    }

    /// Get the spline name.
    pub fn get_spline_name(&self) -> &Token {
        &self.spline_name
    }

    // =========================================================================
    // Interpolation Attribute
    // =========================================================================

    /// Get the interpolation attribute.
    ///
    /// Allowed values: linear, constant, bspline, catmullRom
    pub fn get_interpolation_attr(&self) -> Option<Attribute> {
        let name = self.get_scoped_property_name(&USD_RI_TOKENS.interpolation);
        self.prim.get_attribute(name.as_str())
    }

    /// Create the interpolation attribute.
    ///
    /// Creates a token-typed attribute with uniform variability.
    /// Matches C++ _CreateAttr(name, Token, custom=false, Uniform).
    pub fn create_interpolation_attr(&self) -> Option<Attribute> {
        let name = self.get_scoped_property_name(&USD_RI_TOKENS.interpolation);
        self.prim.create_attribute(
            name.as_str(),
            &usd_sdf::value_type_registry::ValueTypeRegistry::instance().find_type("token"),
            false,
            Some(Variability::Uniform),
        )
    }

    // =========================================================================
    // Positions Attribute
    // =========================================================================

    /// Get the positions attribute.
    ///
    /// Positions of the knots as a float array.
    pub fn get_positions_attr(&self) -> Option<Attribute> {
        let name = self.get_scoped_property_name(&USD_RI_TOKENS.positions);
        self.prim.get_attribute(name.as_str())
    }

    /// Create the positions attribute.
    ///
    /// Creates a float-array-typed attribute with uniform variability.
    /// Matches C++ _CreateAttr(name, FloatArray, custom=false, Uniform).
    pub fn create_positions_attr(&self) -> Option<Attribute> {
        let name = self.get_scoped_property_name(&USD_RI_TOKENS.positions);
        self.prim.create_attribute(
            name.as_str(),
            &usd_sdf::value_type_registry::ValueTypeRegistry::instance().find_type("float[]"),
            false,
            Some(Variability::Uniform),
        )
    }

    // =========================================================================
    // Values Attribute
    // =========================================================================

    /// Get the values attribute.
    ///
    /// Values of the knots (type depends on GetValuesTypeName).
    pub fn get_values_attr(&self) -> Option<Attribute> {
        let name = self.get_scoped_property_name(&USD_RI_TOKENS.values);
        self.prim.get_attribute(name.as_str())
    }

    /// Create the values attribute.
    ///
    /// Creates an attribute with the configured values type and uniform variability.
    /// Matches C++ _CreateAttr(name, _valuesTypeName, custom=false, Uniform).
    pub fn create_values_attr(&self) -> Option<Attribute> {
        let name = self.get_scoped_property_name(&USD_RI_TOKENS.values);
        self.prim.create_attribute(
            name.as_str(),
            &self.values_type_name,
            false,
            Some(Variability::Uniform),
        )
    }

    // =========================================================================
    // Validation
    // =========================================================================

    /// Validates the attribute values belonging to the spline.
    ///
    /// Returns Ok if valid, Err with reason if invalid.
    ///
    /// Validations performed (matching C++):
    /// - The SplineAPI must be fully initialized
    /// - Values type must be FloatArray or Color3fArray
    /// - Interpolation attribute must exist and use an allowed value
    /// - The positions array must be a float array
    /// - The positions array must be sorted by increasing value
    /// - The values array must use the correct value type
    /// - The positions and values array must have the same size
    pub fn validate(&self) -> Result<(), String> {
        let registry = usd_sdf::value_type_registry::ValueTypeRegistry::instance();
        let float_array = registry.find_type("float[]");
        let color3f_array = registry.find_type("color3f[]");

        // Check spline name is set
        if self.spline_name.is_empty() {
            return Err("SplineAPI is not correctly initialized".to_string());
        }

        // Check configured values type is supported
        if self.values_type_name != float_array && self.values_type_name != color3f_array {
            return Err(format!(
                "SplineAPI is configured for an unsupported value type '{}'",
                self.values_type_name.as_token().as_str()
            ));
        }

        // Check interpolation exists
        let interp_attr = self
            .get_interpolation_attr()
            .ok_or("Could not get the interpolation attribute.")?;

        // Check positions exists
        let pos_attr = self
            .get_positions_attr()
            .ok_or("Could not get the position attribute.")?;

        // Check interpolation value is valid — read as Token first (C++ uses TfToken),
        // fall back to String for flexibility.
        if let Some(val) = interp_attr.get(TimeCode::default()) {
            // Try Token first (matches C++ TfToken storage)
            let interp_token = if let Some(tok) = val.downcast_clone::<Token>() {
                tok
            } else if let Some(s) = val.downcast_clone::<String>() {
                Token::new(&s)
            } else {
                Token::new("")
            };

            if !interp_token.is_empty()
                && interp_token != USD_RI_TOKENS.constant
                && interp_token != USD_RI_TOKENS.linear
                && interp_token != USD_RI_TOKENS.catmull_rom
                && interp_token != USD_RI_TOKENS.bspline
            {
                return Err(format!(
                    "Interpolation attribute has invalid value '{}'",
                    interp_token.as_str()
                ));
            }
        }

        // Check positions type is FloatArray
        if pos_attr.get_type_name() != float_array {
            return Err(format!(
                "Positions attribute has incorrect type; found '{}' but expected '{}'",
                pos_attr.get_type_name().as_token().as_str(),
                float_array.as_token().as_str()
            ));
        }

        // Check positions are sorted in increasing order
        if let Some(pos_val) = pos_attr.get(TimeCode::default()) {
            if let Some(positions) = pos_val.as_vec_clone::<f32>() {
                if !positions.windows(2).all(|w| w[0] <= w[1]) {
                    return Err(
                        "Positions attribute must be sorted in increasing order".to_string()
                    );
                }

                // Check values attr exists and type matches
                let val_attr = self
                    .get_values_attr()
                    .ok_or("Could not get the values attribute.")?;

                if val_attr.get_type_name() != self.values_type_name {
                    return Err(format!(
                        "Values attribute has incorrect type; found '{}' but expected '{}'",
                        val_attr.get_type_name().as_token().as_str(),
                        self.values_type_name.as_token().as_str()
                    ));
                }

                // Check positions and values have same size
                let num_values = if self.values_type_name == float_array {
                    val_attr
                        .get(TimeCode::default())
                        .and_then(|v| v.as_vec_clone::<f32>())
                        .map(|v| v.len())
                        .unwrap_or(0)
                } else {
                    // Color3fArray: Vec<[f32; 3]>
                    val_attr
                        .get(TimeCode::default())
                        .and_then(|v| v.as_vec_clone::<[f32; 3]>())
                        .map(|v| v.len())
                        .unwrap_or(0)
                };

                if positions.len() != num_values {
                    return Err(
                        "Values attribute and positions attribute must have the same number of entries".to_string(),
                    );
                }
            }
        }

        Ok(())
    }

    // =========================================================================
    // Helper Methods
    // =========================================================================

    /// Returns the properly-scoped form of the given property name.
    fn get_scoped_property_name(&self, base_name: &Token) -> Token {
        if self.spline_name.is_empty() {
            base_name.clone()
        } else {
            Token::new(&format!(
                "{}:{}:{}",
                self.spline_name.as_str(),
                USD_RI_TOKENS.spline.as_str(),
                base_name.as_str()
            ))
        }
    }

    // =========================================================================
    // Schema attribute names
    // =========================================================================

    /// Returns all pre-declared attributes for this schema class.
    pub fn get_schema_attribute_names(_include_inherited: bool) -> Vec<Token> {
        // Spline API has no fixed attribute names - they depend on spline name
        Vec::new()
    }
}

impl From<Prim> for RiSplineAPI {
    fn from(prim: Prim) -> Self {
        Self::new(prim)
    }
}

impl From<RiSplineAPI> for Prim {
    fn from(api: RiSplineAPI) -> Self {
        api.prim
    }
}

impl AsRef<Prim> for RiSplineAPI {
    fn as_ref(&self) -> &Prim {
        &self.prim
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_schema_type_name() {
        assert_eq!(RiSplineAPI::SCHEMA_TYPE_NAME, "RiSplineAPI");
    }

    #[test]
    fn test_scoped_property_name() {
        let prim = Prim::invalid();
        let api = RiSplineAPI::with_spline_name(
            prim,
            Token::new("mySpline"),
            ValueTypeName::default(),
            false,
        );
        let name = api.get_scoped_property_name(&Token::new("positions"));
        assert_eq!(name.as_str(), "mySpline:spline:positions");
    }
}
