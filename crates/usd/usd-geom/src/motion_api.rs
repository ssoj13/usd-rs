//! UsdGeomMotionAPI - motion blur API schema.
//!
//! Port of pxr/usd/usdGeom/motionAPI.h/cpp
//!
//! API schema for motion blur properties.

use super::tokens::usd_geom_tokens;
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
    /// Matches C++ `CreateMotionBlurScaleAttr()`.
    pub fn create_motion_blur_scale_attr(
        &self,
        default_value: Option<Value>,
        _write_sparsely: bool,
    ) -> Attribute {
        let tokens = usd_geom_tokens();
        let attr_name = tokens.motion_blur_scale.as_str();
        let type_registry = ValueTypeRegistry::instance();
        let value_type = type_registry.find_type_by_token(&Token::new("float"));

        if let Some(attr) = self.prim().create_attribute(
            attr_name,
            &value_type,
            false, // not custom
            None,  // variability defaults to varying
        ) {
            if let Some(default_val) = default_value {
                let _ = attr.set(default_val, TimeCode::default());
            }
            attr
        } else {
            Attribute::invalid()
        }
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
    /// Matches C++ `CreateMotionVelocityScaleAttr()`.
    pub fn create_motion_velocity_scale_attr(
        &self,
        default_value: Option<Value>,
        _write_sparsely: bool,
    ) -> Attribute {
        let tokens = usd_geom_tokens();
        let attr_name = tokens.motion_velocity_scale.as_str();
        let type_registry = ValueTypeRegistry::instance();
        let value_type = type_registry.find_type_by_token(&Token::new("float"));

        if let Some(attr) = self.prim().create_attribute(
            attr_name,
            &value_type,
            false, // not custom
            None,  // variability defaults to varying
        ) {
            if let Some(default_val) = default_value {
                let _ = attr.set(default_val, TimeCode::default());
            }
            attr
        } else {
            Attribute::invalid()
        }
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
    /// Matches C++ `CreateMotionNonlinearSampleCountAttr()`.
    pub fn create_motion_nonlinear_sample_count_attr(
        &self,
        default_value: Option<Value>,
        _write_sparsely: bool,
    ) -> Attribute {
        let tokens = usd_geom_tokens();
        let attr_name = tokens.motion_nonlinear_sample_count.as_str();
        let type_registry = ValueTypeRegistry::instance();
        let value_type = type_registry.find_type_by_token(&Token::new("int"));

        if let Some(attr) = self.prim().create_attribute(
            attr_name,
            &value_type,
            false, // not custom
            None,  // variability defaults to varying
        ) {
            if let Some(default_val) = default_value {
                let _ = attr.set(default_val, TimeCode::default());
            }
            attr
        } else {
            Attribute::invalid()
        }
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
