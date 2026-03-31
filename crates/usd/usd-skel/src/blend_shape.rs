//! UsdSkelBlendShape - describes a target blend shape, possibly containing
//! inbetween shapes.
//!
//! Port of pxr/usd/usdSkel/blendShape.h/cpp

use super::inbetween_shape::InbetweenShape;
use super::tokens::tokens;
use usd_core::{Attribute, Prim, Stage, Typed};
use usd_sdf::{Path, TimeCode};
use usd_tf::Token;
use usd_vt::Value;

// ============================================================================
// BlendShape
// ============================================================================

/// Describes a target blend shape, possibly containing inbetween shapes.
///
/// Matches C++ `UsdSkelBlendShape`.
#[derive(Debug, Clone)]
pub struct BlendShape {
    /// Base typed schema.
    inner: Typed,
}

impl BlendShape {
    /// Creates a BlendShape schema from a prim.
    ///
    /// Matches C++ `UsdSkelBlendShape(const UsdPrim& prim)`.
    pub fn new(prim: Prim) -> Self {
        Self {
            inner: Typed::new(prim),
        }
    }

    /// Creates a BlendShape schema from a Typed schema.
    ///
    /// Matches C++ `UsdSkelBlendShape(const UsdSchemaBase& schemaObj)`.
    pub fn from_typed(typed: Typed) -> Self {
        Self { inner: typed }
    }

    /// Creates an invalid BlendShape schema.
    pub fn invalid() -> Self {
        Self {
            inner: Typed::invalid(),
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

    /// Returns the typed base.
    pub fn typed(&self) -> &Typed {
        &self.inner
    }

    /// Returns the schema type name.
    pub fn schema_type_name() -> Token {
        tokens().blend_shape.clone()
    }

    /// Return a BlendShape holding the prim adhering to this schema at path on stage.
    ///
    /// Matches C++ `UsdSkelBlendShape::Get(const UsdStagePtr &stage, const SdfPath &path)`.
    pub fn get(stage: &Stage, path: &Path) -> Self {
        if let Some(prim) = stage.get_prim_at_path(path) {
            Self::new(prim)
        } else {
            Self::invalid()
        }
    }

    /// Attempt to ensure a prim adhering to this schema at path is defined on this stage.
    ///
    /// Matches C++ `UsdSkelBlendShape::Define(const UsdStagePtr &stage, const SdfPath &path)`.
    pub fn define(stage: &Stage, path: &Path) -> Self {
        match stage.define_prim(path.get_string(), Self::schema_type_name().as_str()) {
            Ok(prim) => Self::new(prim),
            Err(_) => Self::invalid(),
        }
    }

    /// Return a vector of names of all pre-declared attributes for this schema
    /// class and all its ancestor classes.
    ///
    /// Matches C++ `GetSchemaAttributeNames(bool includeInherited)`.
    pub fn get_schema_attribute_names(include_inherited: bool) -> Vec<Token> {
        let local_names = vec![
            tokens().offsets.clone(),
            tokens().normal_offsets.clone(),
            tokens().point_indices.clone(),
        ];

        if include_inherited {
            let mut all_names = Typed::get_schema_attribute_names(true);
            all_names.extend(local_names);
            all_names
        } else {
            local_names
        }
    }

    // ========================================================================
    // OFFSETS
    // ========================================================================

    /// Returns the offsets attribute.
    ///
    /// **Required property**. Position offsets which, when added to the
    /// base pose, provides the target shape.
    ///
    /// Matches C++ `GetOffsetsAttr()`.
    pub fn get_offsets_attr(&self) -> Attribute {
        self.prim()
            .get_attribute(tokens().offsets.as_str())
            .unwrap_or_else(Attribute::invalid)
    }

    /// Creates the offsets attribute.
    ///
    /// Matches C++ `CreateOffsetsAttr(VtValue const &defaultValue, bool writeSparsely)`.
    pub fn create_offsets_attr(
        &self,
        default_value: Option<Value>,
        write_sparsely: bool,
    ) -> Attribute {
        let prim = self.prim();
        if !prim.is_valid() {
            return Attribute::invalid();
        }

        if prim.has_authored_attribute(tokens().offsets.as_str()) {
            if let Some(attr) = prim.get_attribute(tokens().offsets.as_str()) {
                if let Some(default_val) = default_value {
                    if !write_sparsely {
                        let _ = attr.set(default_val.clone(), TimeCode::default());
                    }
                }
                return attr;
            }
        }

        let registry = usd_sdf::ValueTypeRegistry::instance();
        let vec3f_array_type = registry.find_type("vector3f[]");

        if let Some(attr) = prim.create_attribute(
            tokens().offsets.as_str(),
            &vec3f_array_type,
            false,
            Some(usd_core::attribute::Variability::Uniform),
        ) {
            if let Some(default_val) = default_value {
                if !write_sparsely {
                    let _ = attr.set(default_val.clone(), TimeCode::default());
                }
            }
            attr
        } else {
            Attribute::invalid()
        }
    }

    // ========================================================================
    // NORMALOFFSETS
    // ========================================================================

    /// Returns the normalOffsets attribute.
    ///
    /// **Required property**. Normal offsets which, when added to the
    /// base pose, provides the normals of the target shape.
    ///
    /// Matches C++ `GetNormalOffsetsAttr()`.
    pub fn get_normal_offsets_attr(&self) -> Attribute {
        self.prim()
            .get_attribute(tokens().normal_offsets.as_str())
            .unwrap_or_else(Attribute::invalid)
    }

    /// Creates the normalOffsets attribute.
    ///
    /// Matches C++ `CreateNormalOffsetsAttr(VtValue const &defaultValue, bool writeSparsely)`.
    pub fn create_normal_offsets_attr(
        &self,
        default_value: Option<Value>,
        write_sparsely: bool,
    ) -> Attribute {
        let prim = self.prim();
        if !prim.is_valid() {
            return Attribute::invalid();
        }

        if prim.has_authored_attribute(tokens().normal_offsets.as_str()) {
            if let Some(attr) = prim.get_attribute(tokens().normal_offsets.as_str()) {
                if let Some(default_val) = default_value {
                    if !write_sparsely {
                        let _ = attr.set(default_val.clone(), TimeCode::default());
                    }
                }
                return attr;
            }
        }

        let registry = usd_sdf::ValueTypeRegistry::instance();
        let vec3f_array_type = registry.find_type("vector3f[]");

        if let Some(attr) = prim.create_attribute(
            tokens().normal_offsets.as_str(),
            &vec3f_array_type,
            false,
            Some(usd_core::attribute::Variability::Uniform),
        ) {
            if let Some(default_val) = default_value {
                if !write_sparsely {
                    let _ = attr.set(default_val.clone(), TimeCode::default());
                }
            }
            attr
        } else {
            Attribute::invalid()
        }
    }

    // ========================================================================
    // POINTINDICES
    // ========================================================================

    /// Returns the pointIndices attribute.
    ///
    /// **Optional property**. Indices into the original mesh that
    /// correspond to the values in *offsets* and of any inbetween shapes.
    ///
    /// Matches C++ `GetPointIndicesAttr()`.
    pub fn get_point_indices_attr(&self) -> Attribute {
        self.prim()
            .get_attribute(tokens().point_indices.as_str())
            .unwrap_or_else(Attribute::invalid)
    }

    /// Creates the pointIndices attribute.
    ///
    /// Matches C++ `CreatePointIndicesAttr(VtValue const &defaultValue, bool writeSparsely)`.
    pub fn create_point_indices_attr(
        &self,
        default_value: Option<Value>,
        write_sparsely: bool,
    ) -> Attribute {
        let prim = self.prim();
        if !prim.is_valid() {
            return Attribute::invalid();
        }

        if prim.has_authored_attribute(tokens().point_indices.as_str()) {
            if let Some(attr) = prim.get_attribute(tokens().point_indices.as_str()) {
                if let Some(default_val) = default_value {
                    if !write_sparsely {
                        let _ = attr.set(default_val.clone(), TimeCode::default());
                    }
                }
                return attr;
            }
        }

        let registry = usd_sdf::ValueTypeRegistry::instance();
        let int_array_type = registry.find_type("int[]");

        if let Some(attr) = prim.create_attribute(
            tokens().point_indices.as_str(),
            &int_array_type,
            false,
            Some(usd_core::attribute::Variability::Uniform),
        ) {
            if let Some(default_val) = default_value {
                if !write_sparsely {
                    let _ = attr.set(default_val.clone(), TimeCode::default());
                }
            }
            attr
        } else {
            Attribute::invalid()
        }
    }

    // ========================================================================
    // CUSTOM CODE - INBETWEENS
    // ========================================================================

    /// Inbetween shape namespace prefix.
    const INBETWEEN_NAMESPACE: &'static str = "inbetweens:";

    /// Author scene description to create an attribute on this prim that
    /// will be recognized as an Inbetween.
    ///
    /// Matches C++ `CreateInbetween(const TfToken& name)`.
    pub fn create_inbetween(&self, name: &Token) -> InbetweenShape {
        if !self.is_valid() {
            return InbetweenShape::invalid();
        }

        // Construct namespaced attribute name
        let attr_name = format!("{}{}", Self::INBETWEEN_NAMESPACE, name.as_str());

        // Create the attribute
        let registry = usd_sdf::ValueTypeRegistry::instance();
        let vec3f_array_type = registry.find_type("vector3f[]");

        if let Some(attr) = self.prim().create_attribute(
            &attr_name,
            &vec3f_array_type,
            false,
            Some(usd_core::attribute::Variability::Uniform),
        ) {
            InbetweenShape::new(attr)
        } else {
            InbetweenShape::invalid()
        }
    }

    /// Return the Inbetween corresponding to the attribute named `name`.
    ///
    /// Matches C++ `GetInbetween(const TfToken& name)`.
    pub fn get_inbetween(&self, name: &Token) -> InbetweenShape {
        if !self.is_valid() {
            return InbetweenShape::invalid();
        }

        // Try with namespace prefix first
        let namespaced_name = format!("{}{}", Self::INBETWEEN_NAMESPACE, name.as_str());
        if let Some(attr) = self.prim().get_attribute(&namespaced_name) {
            return InbetweenShape::new(attr);
        }

        // Try without namespace prefix
        if let Some(attr) = self.prim().get_attribute(name.as_str()) {
            let inbetween = InbetweenShape::new(attr);
            if inbetween.is_inbetween() {
                return inbetween;
            }
        }

        InbetweenShape::invalid()
    }

    /// Return true if there is a defined Inbetween named `name` on this prim.
    ///
    /// Matches C++ `HasInbetween(const TfToken& name)`.
    pub fn has_inbetween(&self, name: &Token) -> bool {
        self.get_inbetween(name).is_valid()
    }

    /// Return valid InbetweenShape objects for all defined Inbetweens on this prim.
    ///
    /// Matches C++ `GetInbetweens()`.
    pub fn get_inbetweens(&self) -> Vec<InbetweenShape> {
        if !self.is_valid() {
            return Vec::new();
        }

        let mut result = Vec::new();
        for attr_name in self.prim().get_attribute_names() {
            if attr_name.as_str().starts_with(Self::INBETWEEN_NAMESPACE) {
                if let Some(attr) = self.prim().get_attribute(attr_name.as_str()) {
                    let inbetween = InbetweenShape::new(attr);
                    if inbetween.is_valid() {
                        result.push(inbetween);
                    }
                }
            }
        }
        result
    }

    /// Like GetInbetweens(), but exclude inbetweens that have no authored scene description.
    ///
    /// Matches C++ `GetAuthoredInbetweens()`.
    pub fn get_authored_inbetweens(&self) -> Vec<InbetweenShape> {
        if !self.is_valid() {
            return Vec::new();
        }

        // For now, same as get_inbetweens - proper authored check would require
        // checking if each attribute has authored values
        let mut result = Vec::new();
        for attr_name in self.prim().get_attribute_names() {
            if attr_name.as_str().starts_with(Self::INBETWEEN_NAMESPACE) {
                if let Some(attr) = self.prim().get_attribute(attr_name.as_str()) {
                    // Check if attribute has authored value
                    if attr.has_authored_value() {
                        let inbetween = InbetweenShape::new(attr);
                        if inbetween.is_valid() {
                            result.push(inbetween);
                        }
                    }
                }
            }
        }
        result
    }

    /// Validates a set of point indices for a given point count.
    ///
    /// Matches C++ `ValidatePointIndices(TfSpan<const int> indices, size_t numPoints, std::string* reason)`.
    pub fn validate_point_indices(indices: &[i32], num_points: usize) -> Result<(), String> {
        for (i, &idx) in indices.iter().enumerate() {
            if idx < 0 {
                return Err(format!(
                    "Point index at position {} is negative ({})",
                    i, idx
                ));
            }
            if (idx as usize) >= num_points {
                return Err(format!(
                    "Point index at position {} ({}) exceeds point count ({})",
                    i, idx, num_points
                ));
            }
        }
        Ok(())
    }
}

impl PartialEq for BlendShape {
    fn eq(&self, other: &Self) -> bool {
        self.prim() == other.prim()
    }
}

impl Eq for BlendShape {}

impl std::hash::Hash for BlendShape {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.prim().hash(state);
    }
}
