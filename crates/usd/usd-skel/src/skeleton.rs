//! UsdSkelSkeleton - describes a skeleton.
//!
//! Port of pxr/usd/usdSkel/skeleton.h/cpp

use usd_core::{Attribute, Prim, Stage};
use usd_sdf::Path;
use usd_tf::Token;

use super::tokens::tokens;
use usd_geom::boundable::Boundable;

// ============================================================================
// Skeleton
// ============================================================================

/// Describes a skeleton.
///
/// Matches C++ `UsdSkelSkeleton`.
#[derive(Debug, Clone)]
pub struct Skeleton {
    /// Base boundable schema.
    inner: Boundable,
}

impl Skeleton {
    /// Creates a Skeleton schema from a prim.
    ///
    /// Matches C++ `UsdSkelSkeleton(const UsdPrim& prim)`.
    pub fn new(prim: Prim) -> Self {
        Self {
            inner: Boundable::new(prim),
        }
    }

    /// Creates a Skeleton schema from a Boundable schema.
    ///
    /// Matches C++ `UsdSkelSkeleton(const UsdSchemaBase& schemaObj)`.
    pub fn from_boundable(boundable: Boundable) -> Self {
        Self { inner: boundable }
    }

    /// Creates an invalid Skeleton schema.
    pub fn invalid() -> Self {
        Self {
            inner: Boundable::invalid(),
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

    /// Returns the boundable base.
    pub fn boundable(&self) -> &Boundable {
        &self.inner
    }

    /// Returns the schema type name.
    pub fn schema_type_name() -> Token {
        tokens().skeleton.clone()
    }

    /// Return a Skeleton holding the prim adhering to this schema at path on stage.
    ///
    /// Matches C++ `UsdSkelSkeleton::Get(const UsdStagePtr &stage, const SdfPath &path)`.
    pub fn get(stage: &Stage, path: &Path) -> Self {
        if let Some(prim) = stage.get_prim_at_path(path) {
            Self::new(prim)
        } else {
            Self::invalid()
        }
    }

    /// Attempt to ensure a prim adhering to this schema at path is defined on this stage.
    ///
    /// Matches C++ `UsdSkelSkeleton::Define(const UsdStagePtr &stage, const SdfPath &path)`.
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
            tokens().joints.clone(),
            tokens().joint_names.clone(),
            tokens().bind_transforms.clone(),
            tokens().rest_transforms.clone(),
        ];

        if include_inherited {
            let mut all_names = Boundable::get_schema_attribute_names(true);
            all_names.extend(local_names);
            all_names
        } else {
            local_names
        }
    }

    // ========================================================================
    // JOINTS
    // ========================================================================

    /// Returns the joints attribute.
    ///
    /// An array of path tokens identifying the set of joints that make
    /// up the skeleton, and their order. Each token in the array must be valid
    /// when parsed as an SdfPath.
    ///
    /// Matches C++ `GetJointsAttr()`.
    pub fn get_joints_attr(&self) -> Attribute {
        self.prim()
            .get_attribute(tokens().joints.as_str())
            .unwrap_or_else(Attribute::invalid)
    }

    /// Create the joints attribute.
    ///
    /// Matches C++ `CreateJointsAttr(VtValue const &defaultValue, bool writeSparsely)`.
    pub fn create_joints_attr(
        &self,
        default_value: Option<usd_vt::Value>,
        write_sparsely: bool,
    ) -> Option<Attribute> {
        let token_array_type = usd_sdf::ValueTypeRegistry::instance().find_type("token[]");
        let attr = self.prim().create_attribute(
            tokens().joints.as_str(),
            &token_array_type,
            /* custom */ false,
            Some(usd_core::attribute::Variability::Uniform),
        );
        if let Some(ref a) = attr {
            if let Some(val) = default_value {
                let _ = a.set(val, usd_sdf::TimeCode::default());
            }
        }
        // Note: write_sparsely is handled by USD internally
        let _ = write_sparsely; // Suppress unused warning
        attr
    }

    // ========================================================================
    // JOINTNAMES
    // ========================================================================

    /// Returns the jointNames attribute.
    ///
    /// If authored, provides a unique name per joint.
    ///
    /// Matches C++ `GetJointNamesAttr()`.
    pub fn get_joint_names_attr(&self) -> Attribute {
        self.prim()
            .get_attribute(tokens().joint_names.as_str())
            .unwrap_or_else(Attribute::invalid)
    }

    /// Create the jointNames attribute.
    ///
    /// Matches C++ `CreateJointNamesAttr(VtValue const &defaultValue, bool writeSparsely)`.
    pub fn create_joint_names_attr(
        &self,
        default_value: Option<usd_vt::Value>,
        write_sparsely: bool,
    ) -> Option<Attribute> {
        let token_array_type = usd_sdf::ValueTypeRegistry::instance().find_type("token[]");
        let attr = self.prim().create_attribute(
            tokens().joint_names.as_str(),
            &token_array_type,
            /* custom */ false,
            Some(usd_core::attribute::Variability::Uniform),
        );
        if let Some(ref a) = attr {
            if let Some(val) = default_value {
                let _ = a.set(val, usd_sdf::TimeCode::default());
            }
        }
        let _ = write_sparsely;
        attr
    }

    // ========================================================================
    // BINDTRANSFORMS
    // ========================================================================

    /// Returns the bindTransforms attribute.
    ///
    /// Specifies the bind-pose transforms of each joint in
    /// **world space**, in the ordering imposed by *joints*.
    ///
    /// Matches C++ `GetBindTransformsAttr()`.
    pub fn get_bind_transforms_attr(&self) -> Attribute {
        self.prim()
            .get_attribute(tokens().bind_transforms.as_str())
            .unwrap_or_else(Attribute::invalid)
    }

    /// Create the bindTransforms attribute.
    ///
    /// Matches C++ `CreateBindTransformsAttr(VtValue const &defaultValue, bool writeSparsely)`.
    pub fn create_bind_transforms_attr(
        &self,
        default_value: Option<usd_vt::Value>,
        write_sparsely: bool,
    ) -> Option<Attribute> {
        let matrix4d_array_type = usd_sdf::ValueTypeRegistry::instance().find_type("matrix4d[]");
        let attr = self.prim().create_attribute(
            tokens().bind_transforms.as_str(),
            &matrix4d_array_type,
            /* custom */ false,
            Some(usd_core::attribute::Variability::Uniform),
        );
        if let Some(ref a) = attr {
            if let Some(val) = default_value {
                let _ = a.set(val, usd_sdf::TimeCode::default());
            }
        }
        let _ = write_sparsely;
        attr
    }

    // ========================================================================
    // RESTTRANSFORMS
    // ========================================================================

    /// Returns the restTransforms attribute.
    ///
    /// Specifies the rest-pose transforms of each joint in
    /// **local space**, in the ordering imposed by *joints*.
    ///
    /// Matches C++ `GetRestTransformsAttr()`.
    pub fn get_rest_transforms_attr(&self) -> Attribute {
        self.prim()
            .get_attribute(tokens().rest_transforms.as_str())
            .unwrap_or_else(Attribute::invalid)
    }

    /// Create the restTransforms attribute.
    ///
    /// Matches C++ `CreateRestTransformsAttr(VtValue const &defaultValue, bool writeSparsely)`.
    pub fn create_rest_transforms_attr(
        &self,
        default_value: Option<usd_vt::Value>,
        write_sparsely: bool,
    ) -> Option<Attribute> {
        let matrix4d_array_type = usd_sdf::ValueTypeRegistry::instance().find_type("matrix4d[]");
        let attr = self.prim().create_attribute(
            tokens().rest_transforms.as_str(),
            &matrix4d_array_type,
            /* custom */ false,
            Some(usd_core::attribute::Variability::Uniform),
        );
        if let Some(ref a) = attr {
            if let Some(val) = default_value {
                let _ = a.set(val, usd_sdf::TimeCode::default());
            }
        }
        let _ = write_sparsely;
        attr
    }
}

impl PartialEq for Skeleton {
    fn eq(&self, other: &Self) -> bool {
        self.prim() == other.prim()
    }
}

impl Eq for Skeleton {}

impl std::hash::Hash for Skeleton {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.prim().hash(state);
    }
}

impl Default for Skeleton {
    fn default() -> Self {
        Self::invalid()
    }
}
