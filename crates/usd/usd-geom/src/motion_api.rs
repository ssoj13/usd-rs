//! UsdGeomMotionAPI - motion blur API schema.
//!
//! Port of pxr/usd/usdGeom/motionAPI.h/cpp
//!
//! API schema for motion blur properties.

use super::schema_create_default::apply_optional_default;
use super::tokens::usd_geom_tokens;
use usd_core::attribute::Variability;
use usd_core::schema_base::APISchemaBase;
use usd_core::{Attribute, Prim};
use usd_sdf::{TimeCode, ValueTypeRegistry};
use usd_tf::Token;
use usd_vt::Value;

// ============================================================================
// MotionAPI
// ============================================================================

/// API schema for motion blur properties.
///
/// Matches C++ `UsdGeomMotionAPI`.
#[derive(Debug, Clone)]
pub struct MotionAPI {
    /// Base API schema.
    base: APISchemaBase,
}

impl MotionAPI {
    /// Constructs a MotionAPI from a prim.
    ///
    /// Matches C++ `UsdGeomMotionAPI(UsdPrim)`.
    pub fn new(prim: Prim) -> Self {
        Self {
            base: APISchemaBase::new(prim),
        }
    }

    /// Constructs an invalid MotionAPI.
    ///
    /// Matches C++ default constructor.
    pub fn invalid() -> Self {
        Self {
            base: APISchemaBase::invalid(),
        }
    }

    /// Returns true if this schema is valid.
    pub fn is_valid(&self) -> bool {
        self.base.is_valid()
    }

    /// Returns the wrapped prim.
    pub fn prim(&self) -> &Prim {
        self.base.prim()
    }

    /// Returns the schema type name.
    pub fn schema_type_name() -> Token {
        Token::new("MotionAPI")
    }

    /// Returns the schema type name as a string.
    pub fn schema_type_name_str() -> &'static str {
        "MotionAPI"
    }

    /// Returns true if this API schema can be applied to the given prim.
    ///
    /// Matches C++ `CanApply()`.
    pub fn can_apply(prim: &Prim) -> bool {
        // MotionAPI can be applied to any prim
        prim.is_valid()
    }

    /// Applies this API schema to the given prim.
    ///
    /// Matches C++ `Apply()`.
    pub fn apply(prim: &Prim) -> Self {
        // Apply the schema by adding it to apiSchemas metadata
        let schema_type_name = Self::schema_type_name();
        if prim.apply_api(&schema_type_name) {
            Self::new(prim.clone())
        } else {
            Self::invalid()
        }
    }

    /// Gets a MotionAPI from a prim.
    ///
    /// Matches C++ `Get(UsdPrim)`.
    pub fn get(prim: &Prim) -> Self {
        Self::new(prim.clone())
    }

    /// Create-or-get a MotionAPI attribute; optional default at default time.
    ///
    /// Matches C++ `Create*Attr(VtValue defaultValue, bool writeSparsely)`.
    fn create_motion_schema_attr(
        &self,
        name: &str,
        sdf_typename: &str,
        default_value: Option<Value>,
        _write_sparsely: bool,
    ) -> Attribute {
        let prim = self.prim();
        if !prim.is_valid() {
            return Attribute::invalid();
        }
        let attr = if prim.has_authored_attribute(name) {
            prim.get_attribute(name).unwrap_or_else(Attribute::invalid)
        } else {
            let type_registry = ValueTypeRegistry::instance();
            let value_type = type_registry.find_type_by_token(&Token::new(sdf_typename));
            prim.create_attribute(name, &value_type, false, Some(Variability::Varying))
                .unwrap_or_else(Attribute::invalid)
        };
        apply_optional_default(attr, default_value)
    }

    // ========================================================================
    // Motion Blur Attributes
    // ========================================================================

    /// Gets the motion:blurScale attribute.
    ///
    /// Matches C++ `GetMotionBlurScaleAttr()`.
    pub fn get_motion_blur_scale_attr(&self) -> Attribute {
        let tokens = usd_geom_tokens();
        self.prim()
            .get_attribute(tokens.motion_blur_scale.as_str())
            .unwrap_or_else(Attribute::invalid)
    }

    /// Creates the motion:blurScale attribute.
    ///
    /// Matches C++ `CreateMotionBlurScaleAttr(VtValue defaultValue, bool writeSparsely)`.
    pub fn create_motion_blur_scale_attr(
        &self,
        default_value: Option<Value>,
        write_sparsely: bool,
    ) -> Attribute {
        let name = usd_geom_tokens().motion_blur_scale.as_str();
        self.create_motion_schema_attr(name, "float", default_value, write_sparsely)
    }

    /// Gets the motion:velocityScale attribute.
    ///
    /// Matches C++ `GetMotionVelocityScaleAttr()`.
    pub fn get_motion_velocity_scale_attr(&self) -> Attribute {
        let tokens = usd_geom_tokens();
        self.prim()
            .get_attribute(tokens.motion_velocity_scale.as_str())
            .unwrap_or_else(Attribute::invalid)
    }

    /// Creates the motion:velocityScale attribute.
    ///
    /// Matches C++ `CreateMotionVelocityScaleAttr(VtValue defaultValue, bool writeSparsely)`.
    pub fn create_motion_velocity_scale_attr(
        &self,
        default_value: Option<Value>,
        write_sparsely: bool,
    ) -> Attribute {
        let name = usd_geom_tokens().motion_velocity_scale.as_str();
        self.create_motion_schema_attr(name, "float", default_value, write_sparsely)
    }

    /// Gets the motion:nonlinearSampleCount attribute.
    ///
    /// Matches C++ `GetMotionNonlinearSampleCountAttr()`.
    pub fn get_motion_nonlinear_sample_count_attr(&self) -> Attribute {
        let tokens = usd_geom_tokens();
        self.prim()
            .get_attribute(tokens.motion_nonlinear_sample_count.as_str())
            .unwrap_or_else(Attribute::invalid)
    }

    /// Creates the motion:nonlinearSampleCount attribute.
    ///
    /// Matches C++ `CreateMotionNonlinearSampleCountAttr(VtValue defaultValue, bool writeSparsely)`.
    pub fn create_motion_nonlinear_sample_count_attr(
        &self,
        default_value: Option<Value>,
        write_sparsely: bool,
    ) -> Attribute {
        let name = usd_geom_tokens().motion_nonlinear_sample_count.as_str();
        self.create_motion_schema_attr(name, "int", default_value, write_sparsely)
    }

    // ========================================================================
    // Computed (Inherited) Values
    // ========================================================================

    /// Compute the inherited value of velocityScale at `time`.
    ///
    /// Walks up the namespace hierarchy to find the nearest authored value.
    /// Returns 1.0 if no value is authored on the prim or its ancestors.
    ///
    /// Matches C++ `ComputeVelocityScale()`.
    pub fn compute_velocity_scale(&self, time: TimeCode) -> f32 {
        self.compute_inherited_float(usd_geom_tokens().motion_velocity_scale.as_str(), time, 1.0)
    }

    /// Compute the inherited value of nonlinearSampleCount at `time`.
    ///
    /// Walks up the namespace hierarchy to find the nearest authored value.
    /// Returns 3 if no value is authored on the prim or its ancestors.
    ///
    /// Matches C++ `ComputeNonlinearSampleCount()`.
    pub fn compute_nonlinear_sample_count(&self, time: TimeCode) -> i32 {
        self.compute_inherited_int(
            usd_geom_tokens().motion_nonlinear_sample_count.as_str(),
            time,
            3,
        )
    }

    /// Compute the inherited value of motion:blurScale at `time`.
    ///
    /// Walks up the namespace hierarchy to find the nearest authored value.
    /// Returns 1.0 if no value is authored on the prim or its ancestors.
    ///
    /// Matches C++ `ComputeMotionBlurScale()`.
    pub fn compute_motion_blur_scale(&self, time: TimeCode) -> f32 {
        self.compute_inherited_float(usd_geom_tokens().motion_blur_scale.as_str(), time, 1.0)
    }

    // ========================================================================
    // Private Helpers
    // ========================================================================

    /// Walk up namespace to find nearest authored float attribute value.
    fn compute_inherited_float(&self, attr_name: &str, time: TimeCode, fallback: f32) -> f32 {
        let mut prim = self.prim().clone();
        while prim.is_valid() {
            if let Some(attr) = prim.get_attribute(attr_name) {
                if attr.has_authored_value() {
                    if let Some(val) = attr.get(time) {
                        if let Some(f) = val.downcast_clone::<f32>() {
                            return f;
                        }
                    }
                }
            }
            prim = prim.parent();
        }
        fallback
    }

    /// Walk up namespace to find nearest authored int attribute value.
    fn compute_inherited_int(&self, attr_name: &str, time: TimeCode, fallback: i32) -> i32 {
        let mut prim = self.prim().clone();
        while prim.is_valid() {
            if let Some(attr) = prim.get_attribute(attr_name) {
                if attr.has_authored_value() {
                    if let Some(val) = attr.get(time) {
                        if let Some(i) = val.downcast_clone::<i32>() {
                            return i;
                        }
                    }
                }
            }
            prim = prim.parent();
        }
        fallback
    }
}

impl PartialEq for MotionAPI {
    fn eq(&self, other: &Self) -> bool {
        self.base == other.base
    }
}

impl Eq for MotionAPI {}

#[cfg(test)]
mod tests {
    use super::MotionAPI;
    use std::sync::Arc;
    use usd_core::{InitialLoadSet, Stage};
    use usd_sdf::TimeCode;
    use usd_vt::Value;

    #[test]
    fn create_motion_blur_scale_default_float() {
        let _ = usd_sdf::init();
        let stage = Arc::new(Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap());
        let prim = stage.define_prim("/MotionBlur", "Xform").unwrap();
        let api = MotionAPI::new(prim);
        let attr = api.create_motion_blur_scale_attr(Some(Value::from_f32(0.5)), false);
        assert!(attr.is_valid());
        assert_eq!(attr.get_typed::<f32>(TimeCode::default()), Some(0.5));
    }

    #[test]
    fn create_motion_nonlinear_sample_count_default_int() {
        let _ = usd_sdf::init();
        let stage = Arc::new(Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap());
        let prim = stage.define_prim("/MotionNl", "Xform").unwrap();
        let api = MotionAPI::new(prim);
        let attr = api.create_motion_nonlinear_sample_count_attr(Some(Value::new(7i32)), false);
        assert!(attr.is_valid());
        assert_eq!(attr.get_typed::<i32>(TimeCode::default()), Some(7));
    }
}
