//! UsdGeomConstraintTarget - constraint target schema wrapper.
//!
//! Port of pxr/usd/usdGeom/constraintTarget.h/cpp
//!
//! Schema wrapper for UsdAttribute for authoring and introspecting
//! attributes that are constraint targets.

use super::tokens::usd_geom_tokens;
use super::xform_cache::XformCache;
use usd_core::Attribute;
use usd_gf::matrix4::Matrix4d;
use usd_sdf::TimeCode;
use usd_tf::Token;

// ============================================================================
// ConstraintTarget
// ============================================================================

/// Schema wrapper for UsdAttribute for authoring and introspecting
/// attributes that are constraint targets.
///
/// Constraint targets correspond roughly to what some DCC's call locators.
/// They are coordinate frames, represented as (animated or static) GfMatrix4d
/// values.  We represent them as attributes in USD rather than transformable
/// prims because generally we require no other coordinated information about
/// a constraint target other than its name and its matrix value, and because
/// attributes are more concise than prims.
///
/// Because consumer clients often care only about the identity and value of
/// constraint targets and may be able to usefully consume them without caring
/// about the actual geometry with which they may logically correspond,
/// UsdGeom aggregates all constraint targets onto a model's root prim,
/// assuming that an exporter will use property namespacing within the
/// constraint target attribute's name to indicate a path to a prim within
/// the model with which the constraint target may correspond.
///
/// To facilitate instancing, and also position-tweaking of baked assets, we
/// stipulate that constraint target values always be recorded in
/// **model-relative transformation space**.  In other words, to get the
/// world-space value of a constraint target, transform it by the
/// local-to-world transformation of the prim on which it is recorded.  
/// ComputeInWorldSpace() will perform this calculation.
///
/// Matches C++ `UsdGeomConstraintTarget`.
#[derive(Debug, Clone)]
pub struct ConstraintTarget {
    /// Wrapped attribute.
    attr: Attribute,
}

impl ConstraintTarget {
    /// Creates an invalid ConstraintTarget.
    ///
    /// Matches C++ default constructor.
    pub fn invalid() -> Self {
        Self {
            attr: Attribute::invalid(),
        }
    }

    /// Creates a ConstraintTarget from an attribute.
    ///
    /// Speculative constructor that will produce a valid
    /// ConstraintTarget when `attr` already represents an attribute
    /// that is a ConstraintTarget, and produces an invalid
    /// ConstraintTarget otherwise.
    ///
    /// Matches C++ `UsdGeomConstraintTarget(const UsdAttribute &attr)`.
    pub fn new(attr: Attribute) -> Self {
        Self { attr }
    }

    /// Test whether a given Attribute represents valid ConstraintTarget.
    ///
    /// Success implies that `attr.is_valid()` is true.
    ///
    /// Matches C++ `IsValid(const UsdAttribute &attr)`.
    pub fn is_valid(attr: &Attribute) -> bool {
        if !attr.is_valid() {
            return false;
        }

        // Check if it's in the constraintTargets namespace
        // The attribute name should start with "constraintTargets:"
        let attr_name = attr.name();
        let attr_name_str = attr_name.as_str();
        let constraint_targets_token = usd_geom_tokens().constraint_targets.as_str();

        // Check if name starts with "constraintTargets:" (namespace prefix)
        if !attr_name_str.starts_with(&format!("{}:", constraint_targets_token)) {
            return false;
        }

        // Check if it's matrix-typed (matrix4d)
        let type_name = attr.type_name();
        let type_name_str = type_name.as_str();

        // Check for matrix4d type
        type_name_str == "matrix4d" || type_name_str.contains("matrix4d")
    }

    /// Returns the wrapped attribute.
    ///
    /// Matches C++ `GetAttr()`.
    pub fn attr(&self) -> &Attribute {
        &self.attr
    }

    /// Return true if the wrapped Attribute is defined, and in
    /// addition the attribute is identified as a ConstraintTarget.
    ///
    /// Matches C++ `IsDefined()`.
    pub fn is_defined(&self) -> bool {
        Self::is_valid(&self.attr)
    }

    /// Explicit bool conversion operator.
    ///
    /// A ConstraintTarget object converts to `true` iff it is valid for
    /// querying and authoring values and metadata (which is identically
    /// equivalent to `is_defined()`). It converts to `false` otherwise.
    ///
    /// Matches C++ explicit `operator bool()`.
    pub fn is_valid_instance(&self) -> bool {
        self.is_defined()
    }

    /// Get the attribute value of the ConstraintTarget at `time`.
    ///
    /// Matches C++ `Get(GfMatrix4d* value, UsdTimeCode time)`.
    pub fn get(&self, time: TimeCode) -> Option<Matrix4d> {
        if !self.is_defined() {
            return None;
        }
        self.attr.get_typed::<Matrix4d>(time)
    }

    /// Set the attribute value of the ConstraintTarget at `time`.
    ///
    /// Matches C++ `Set(const GfMatrix4d& value, UsdTimeCode time)`.
    pub fn set(&self, value: &Matrix4d, time: TimeCode) -> bool {
        if !self.is_defined() {
            return false;
        }
        use usd_vt::Value;
        self.attr.set(Value::from_no_hash(*value), time)
    }

    /// Get the stored identifier unique to the enclosing model's namespace for
    /// this constraint target.
    ///
    /// Matches C++ `GetIdentifier()`.
    pub fn get_identifier(&self) -> Token {
        if !self.is_defined() {
            return Token::new("");
        }

        let identifier_token = usd_geom_tokens().constraint_target_identifier.clone();
        if let Some(metadata_value) = self.attr.get_metadata(&identifier_token) {
            if let Some(token_val) = metadata_value.get::<Token>() {
                return token_val.clone();
            }
            if let Some(str_val) = metadata_value.get::<String>() {
                return Token::new(str_val);
            }
        }

        Token::new("")
    }

    /// Explicitly sets the stored identifier to the given string.
    ///
    /// Clients are responsible for ensuring the uniqueness of this identifier
    /// within the enclosing model's namespace.
    ///
    /// Matches C++ `SetIdentifier(const TfToken &identifier)`.
    pub fn set_identifier(&self, identifier: &Token) -> bool {
        if !self.is_defined() {
            return false;
        }

        let identifier_token = usd_geom_tokens().constraint_target_identifier.clone();
        use usd_vt::Value;
        self.attr
            .set_metadata(&identifier_token, Value::from_no_hash(identifier.clone()))
    }

    /// Returns the fully namespaced constraint attribute name, given the
    /// constraint name.
    ///
    /// Matches C++ `GetConstraintAttrName(const std::string &constraintName)`.
    pub fn get_constraint_attr_name(constraint_name: &str) -> Token {
        let constraint_targets_token = usd_geom_tokens().constraint_targets.as_str();
        Token::new(&format!("{}:{}", constraint_targets_token, constraint_name))
    }

    /// Computes the value of the constraint target in world space.
    ///
    /// If a valid XformCache is provided in the argument `xf_cache`,
    /// it is used to evaluate the CTM of the model to which the constraint
    /// target belongs.
    ///
    /// To get the constraint value in model-space (or local space), simply
    /// use `get()`, since the authored values must already be in model-space.
    ///
    /// Matches C++ `ComputeInWorldSpace(UsdTimeCode time, UsdGeomXformCache *xfCache)`.
    pub fn compute_in_world_space(
        &self,
        time: TimeCode,
        xf_cache: Option<&mut XformCache>,
    ) -> Matrix4d {
        if !self.is_defined() {
            return Matrix4d::identity();
        }

        // Get the model-space value
        let local_val = match self.get(time) {
            Some(v) => v,
            None => return Matrix4d::identity(),
        };

        // Resolve owning prim via Attribute::stage() + prim_path()
        let prim_path = self.attr.prim_path();
        let stage = match self.attr.stage() {
            Some(s) => s,
            None => return local_val, // No stage -- fall back to local
        };
        let prim = match stage.get_prim_at_path(&prim_path) {
            Some(p) => p,
            None => return local_val,
        };

        // Get local-to-world transform from cache or temp cache
        let local_to_world = if let Some(cache) = xf_cache {
            cache.get_local_to_world_transform(&prim)
        } else {
            let mut temp_cache = XformCache::new(time);
            temp_cache.get_local_to_world_transform(&prim)
        };

        // C++ does: localConstraintSpace * localToWorld
        local_val * local_to_world
    }

    /// Computes the value of the constraint target in world space, given the prim.
    ///
    /// This is a convenience method that takes the prim explicitly.
    ///
    /// Matches C++ `ComputeInWorldSpace(UsdTimeCode time, UsdGeomXformCache *xfCache)`
    /// but with explicit prim parameter.
    pub fn compute_in_world_space_with_prim(
        &self,
        prim: &usd_core::Prim,
        time: TimeCode,
        xf_cache: Option<&mut XformCache>,
    ) -> Matrix4d {
        if !self.is_defined() {
            return Matrix4d::identity();
        }

        // Get the model-space value
        let local_constraint_space = match self.get(time) {
            Some(val) => val,
            None => {
                return Matrix4d::identity();
            }
        };

        // Compute the local-to-world transform
        let local_to_world = if let Some(cache) = xf_cache {
            cache.get_local_to_world_transform(prim)
        } else {
            // Create a temporary cache if none provided
            let mut temp_cache = XformCache::new(time);
            temp_cache.get_local_to_world_transform(prim)
        };

        // Transform the model-space value by the local-to-world transform
        // Note: C++ does localConstraintSpace * localToWorld (matrix multiplication order)
        local_constraint_space * local_to_world
    }
}

impl From<Attribute> for ConstraintTarget {
    fn from(attr: Attribute) -> Self {
        Self::new(attr)
    }
}

impl From<&Attribute> for ConstraintTarget {
    fn from(attr: &Attribute) -> Self {
        Self::new(attr.clone())
    }
}

impl PartialEq for ConstraintTarget {
    fn eq(&self, other: &Self) -> bool {
        self.attr == other.attr
    }
}

impl Eq for ConstraintTarget {}
