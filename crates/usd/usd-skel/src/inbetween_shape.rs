//! UsdSkelInbetweenShape - schema wrapper for UsdAttribute for authoring and
//! introspecting attributes that serve as inbetween shapes of a UsdSkelBlendShape.
//!
//! Port of pxr/usd/usdSkel/inbetweenShape.h/cpp

use usd_core::{Attribute, Prim};
use usd_gf::Vec3f;
use usd_sdf::TimeCode;
use usd_tf::Token;
use usd_vt::Value;

// ============================================================================
// InbetweenShape
// ============================================================================

/// Schema wrapper for UsdAttribute for authoring and introspecting attributes
/// that serve as inbetween shapes of a UsdSkelBlendShape.
///
/// Inbetween shapes allow an explicit shape to be specified when the blendshape
/// to which it's bound is evaluated at a certain weight.
///
/// Matches C++ `UsdSkelInbetweenShape`.
#[derive(Debug, Clone)]
pub struct InbetweenShape {
    /// The wrapped attribute.
    attr: Attribute,
}

impl InbetweenShape {
    /// Inbetween namespace prefix.
    const NAMESPACE_PREFIX: &'static str = "inbetweens:";

    /// Normal offsets suffix.
    const NORMAL_OFFSETS_SUFFIX: &'static str = ":normalOffsets";

    /// Weight metadata key.
    const WEIGHT_KEY: &'static str = "inbetweenWeight";

    /// Default constructor returns an invalid inbetween shape.
    pub fn invalid() -> Self {
        Self {
            attr: Attribute::invalid(),
        }
    }

    /// Creates an InbetweenShape from an attribute.
    ///
    /// Matches C++ `UsdSkelInbetweenShape(const UsdAttribute& attr)`.
    pub fn new(attr: Attribute) -> Self {
        Self { attr }
    }

    /// Return true if this InbetweenShape is valid.
    pub fn is_valid(&self) -> bool {
        self.attr.is_valid() && self.is_inbetween()
    }

    /// Return true if this is a valid inbetween.
    ///
    /// Matches C++ `IsDefined()`.
    pub fn is_defined(&self) -> bool {
        self.is_valid()
    }

    /// Returns the wrapped attribute.
    ///
    /// Matches C++ `GetAttr()`.
    pub fn attr(&self) -> &Attribute {
        &self.attr
    }

    /// Test whether a given attribute name represents a valid Inbetween.
    ///
    /// Matches C++ `IsInbetween(const UsdAttribute& attr)`.
    pub fn is_inbetween(&self) -> bool {
        self.attr.is_valid()
            && self
                .attr
                .name()
                .as_str()
                .starts_with(Self::NAMESPACE_PREFIX)
    }

    /// Return the location at which the shape is applied.
    ///
    /// Matches C++ `GetWeight(float* weight)`.
    pub fn get_weight(&self) -> Option<f32> {
        if !self.is_valid() {
            return None;
        }

        // Get weight from custom metadata
        let key = Token::new(Self::WEIGHT_KEY);
        self.attr
            .get_metadata(&key)
            .and_then(|v| v.downcast_clone::<f32>())
    }

    /// Set the location at which the shape is applied.
    ///
    /// Matches C++ `SetWeight(float weight)`.
    pub fn set_weight(&self, weight: f32) -> bool {
        if !self.is_valid() {
            return false;
        }

        let key = Token::new(Self::WEIGHT_KEY);
        self.attr.set_metadata(&key, Value::from(weight))
    }

    /// Has a weight value been explicitly authored on this shape?
    ///
    /// Matches C++ `HasAuthoredWeight()`.
    pub fn has_authored_weight(&self) -> bool {
        if !self.is_valid() {
            return false;
        }

        let key = Token::new(Self::WEIGHT_KEY);
        self.attr.has_authored_metadata(&key)
    }

    /// Get the point offsets corresponding to this shape.
    ///
    /// Matches C++ `GetOffsets(VtVec3fArray* offsets)`.
    pub fn get_offsets(&self) -> Option<Vec<Vec3f>> {
        if !self.is_valid() {
            return None;
        }

        self.attr.get_typed_vec::<Vec3f>(TimeCode::default())
    }

    /// Set the point offsets corresponding to this shape.
    ///
    /// Matches C++ `SetOffsets(const VtVec3fArray& offsets)`.
    pub fn set_offsets(&self, offsets: &[Vec3f]) -> bool {
        if !self.is_valid() {
            return false;
        }

        self.attr
            .set(Value::from_no_hash(offsets.to_vec()), TimeCode::default())
    }

    /// Returns a valid normal offsets attribute if the shape has normal offsets.
    ///
    /// Matches C++ `GetNormalOffsetsAttr()`.
    pub fn get_normal_offsets_attr(&self) -> Attribute {
        self.get_or_create_normal_offsets_attr(false)
    }

    /// Returns the existing normal offsets attribute if the shape has
    /// normal offsets, or creates a new one.
    ///
    /// Matches C++ `CreateNormalOffsetsAttr(const VtValue &defaultValue)`.
    pub fn create_normal_offsets_attr(&self, default_value: Option<Value>) -> Attribute {
        let attr = self.get_or_create_normal_offsets_attr(true);
        if let Some(val) = default_value {
            let _ = attr.set(val, TimeCode::default());
        }
        attr
    }

    /// Get or create normal offsets attribute.
    fn get_or_create_normal_offsets_attr(&self, create: bool) -> Attribute {
        if !self.is_valid() {
            return Attribute::invalid();
        }

        // Get the prim via stage
        let Some(stage) = self.attr.stage() else {
            return Attribute::invalid();
        };
        let prim_path = self.attr.prim_path();
        let Some(prim) = stage.get_prim_at_path(&prim_path) else {
            return Attribute::invalid();
        };

        // Build normal offsets attribute name
        let normal_offsets_name = format!("{}{}", self.attr.name(), Self::NORMAL_OFFSETS_SUFFIX);

        // Try to get existing attribute
        if prim.has_authored_attribute(&normal_offsets_name) {
            return prim.get_attribute(&normal_offsets_name).unwrap();
        }

        // Create if requested
        if create {
            let registry = usd_sdf::ValueTypeRegistry::instance();
            let vec3f_array_type = registry.find_type("vector3f[]");

            if let Some(attr) = prim.create_attribute(
                &normal_offsets_name,
                &vec3f_array_type,
                false,
                None, // C++ uses SdfVariabilityVarying for normalOffsets
            ) {
                return attr;
            }
        }

        Attribute::invalid()
    }

    /// Get the normal offsets authored for this shape.
    ///
    /// Matches C++ `GetNormalOffsets(VtVec3fArray* offsets)`.
    pub fn get_normal_offsets(&self) -> Option<Vec<Vec3f>> {
        let attr = self.get_normal_offsets_attr();
        if !attr.is_valid() {
            return None;
        }

        attr.get_typed_vec::<Vec3f>(TimeCode::default())
    }

    /// Set the normal offsets authored for this shape.
    ///
    /// Matches C++ `SetNormalOffsets(const VtVec3fArray& offsets)`.
    pub fn set_normal_offsets(&self, offsets: &[Vec3f]) -> bool {
        let attr = self.create_normal_offsets_attr(None);
        if !attr.is_valid() {
            return false;
        }

        attr.set(Value::from_no_hash(offsets.to_vec()), TimeCode::default())
    }

    /// Factory method for creating an inbetween shape.
    ///
    /// Matches C++ `_Create(const UsdPrim& prim, const TfToken& name)`.
    pub fn create(prim: &Prim, name: &Token) -> Self {
        if !prim.is_valid() {
            return Self::invalid();
        }

        // Construct namespaced name
        let attr_name = Self::make_namespaced(name);
        if attr_name.is_empty() {
            return Self::invalid();
        }

        // Create the attribute
        let registry = usd_sdf::ValueTypeRegistry::instance();
        let vec3f_array_type = registry.find_type("vector3f[]");

        if let Some(attr) = prim.create_attribute(
            attr_name.as_str(),
            &vec3f_array_type,
            false,
            Some(usd_core::attribute::Variability::Uniform),
        ) {
            Self::new(attr)
        } else {
            Self::invalid()
        }
    }

    /// Returns the namespace prefix for inbetween shapes.
    pub fn namespace_prefix() -> &'static str {
        Self::NAMESPACE_PREFIX
    }

    /// Returns the normal offsets suffix.
    pub fn normal_offsets_suffix() -> &'static str {
        Self::NORMAL_OFFSETS_SUFFIX
    }

    /// Returns name prepended with the proper inbetween namespace.
    fn make_namespaced(name: &Token) -> Token {
        let name_str = name.as_str();

        // If already namespaced, return as-is
        if name_str.starts_with(Self::NAMESPACE_PREFIX) {
            return name.clone();
        }

        // Prepend namespace
        Token::new(&format!("{}{}", Self::NAMESPACE_PREFIX, name_str))
    }
}

impl PartialEq for InbetweenShape {
    fn eq(&self, other: &Self) -> bool {
        self.attr == other.attr
    }
}

impl Eq for InbetweenShape {}

impl std::hash::Hash for InbetweenShape {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.attr.hash(state);
    }
}
