//! UsdSkelAnimation - describes a skel animation, where joint animation
//! is stored in a vectorized form.
//!
//! Port of pxr/usd/usdSkel/animation.h/cpp

use super::tokens::tokens;
use usd_core::{Attribute, Prim, Stage, Typed, attribute::Variability};
use usd_gf::Matrix4d;
use usd_sdf::Path;
use usd_sdf::TimeCode;
use usd_tf::Token;
use usd_vt::Value;

// ============================================================================
// Helper Functions
// ============================================================================

/// Compose a transform matrix from translation, rotation (quaternion), and scale.
fn compose_transform(
    translate: &usd_gf::Vec3d,
    rotate: &usd_gf::Quatf,
    scale: &usd_gf::Vec3d,
) -> Matrix4d {
    // Convert quaternion to rotation matrix elements
    let w = rotate.real() as f64;
    let x = rotate.imaginary().x as f64;
    let y = rotate.imaginary().y as f64;
    let z = rotate.imaginary().z as f64;

    // Rotation matrix from quaternion
    let r00 = 1.0 - 2.0 * (y * y + z * z);
    let r01 = 2.0 * (x * y - z * w);
    let r02 = 2.0 * (x * z + y * w);
    let r10 = 2.0 * (x * y + z * w);
    let r11 = 1.0 - 2.0 * (x * x + z * z);
    let r12 = 2.0 * (y * z - x * w);
    let r20 = 2.0 * (x * z - y * w);
    let r21 = 2.0 * (y * z + x * w);
    let r22 = 1.0 - 2.0 * (x * x + y * y);

    // Compose: T * R * S (row-major storage)
    Matrix4d::new(
        r00 * scale.x,
        r01 * scale.x,
        r02 * scale.x,
        0.0,
        r10 * scale.y,
        r11 * scale.y,
        r12 * scale.y,
        0.0,
        r20 * scale.z,
        r21 * scale.z,
        r22 * scale.z,
        0.0,
        translate.x,
        translate.y,
        translate.z,
        1.0,
    )
}

// ============================================================================
// SkelAnimation
// ============================================================================

/// Describes a skel animation, where joint animation is stored in a vectorized form.
///
/// Matches C++ `UsdSkelAnimation`.
#[derive(Debug, Clone)]
pub struct SkelAnimation {
    /// Base typed schema.
    inner: Typed,
}

impl SkelAnimation {
    /// Creates a SkelAnimation schema from a prim.
    ///
    /// Matches C++ `UsdSkelAnimation(const UsdPrim& prim)`.
    pub fn new(prim: Prim) -> Self {
        Self {
            inner: Typed::new(prim),
        }
    }

    /// Creates a SkelAnimation schema from a Typed schema.
    ///
    /// Matches C++ `UsdSkelAnimation(const UsdSchemaBase& schemaObj)`.
    pub fn from_typed(typed: Typed) -> Self {
        Self { inner: typed }
    }

    /// Creates an invalid SkelAnimation schema.
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
        tokens().skel_animation.clone()
    }

    /// Return a SkelAnimation holding the prim adhering to this schema at path on stage.
    ///
    /// Matches C++ `UsdSkelAnimation::Get(const UsdStagePtr &stage, const SdfPath &path)`.
    pub fn get(stage: &Stage, path: &Path) -> Self {
        if let Some(prim) = stage.get_prim_at_path(path) {
            Self::new(prim)
        } else {
            Self::invalid()
        }
    }

    /// Attempt to ensure a prim adhering to this schema at path is defined on this stage.
    ///
    /// Matches C++ `UsdSkelAnimation::Define(const UsdStagePtr &stage, const SdfPath &path)`.
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
            tokens().translations.clone(),
            tokens().rotations.clone(),
            tokens().scales.clone(),
            tokens().blend_shapes.clone(),
            tokens().blend_shape_weights.clone(),
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
    // JOINTS
    // ========================================================================

    /// Returns the joints attribute.
    ///
    /// Array of tokens identifying which joints this animation's
    /// data applies to. The tokens for joints correspond to the tokens of
    /// Skeleton primitives.
    ///
    /// Matches C++ `GetJointsAttr()`.
    pub fn get_joints_attr(&self) -> Attribute {
        self.prim()
            .get_attribute(tokens().joints.as_str())
            .unwrap_or_else(Attribute::invalid)
    }

    /// Creates the joints attribute.
    ///
    /// Matches C++ `CreateJointsAttr(VtValue const &defaultValue, bool writeSparsely)`.
    pub fn create_joints_attr(
        &self,
        default_value: Option<Value>,
        _write_sparsely: bool,
    ) -> Attribute {
        use usd_sdf::ValueTypeRegistry;

        // joints is uniform per C++ schema (uniform token[] joints)
        let type_name = ValueTypeRegistry::instance().find_type("token[]");
        if let Some(attr) = self.prim().create_attribute(
            tokens().joints.as_str(),
            &type_name,
            false,                      // not custom
            Some(Variability::Uniform), // uniform per C++ schema
        ) {
            if let Some(val) = default_value {
                let _ = attr.set(val, TimeCode::default());
            }
            return attr;
        }

        Attribute::invalid()
    }

    // ========================================================================
    // TRANSLATIONS
    // ========================================================================

    /// Returns the translations attribute.
    ///
    /// Joint-local translations of all affected joints.
    ///
    /// Matches C++ `GetTranslationsAttr()`.
    pub fn get_translations_attr(&self) -> Attribute {
        self.prim()
            .get_attribute(tokens().translations.as_str())
            .unwrap_or_else(Attribute::invalid)
    }

    /// Creates the translations attribute.
    ///
    /// Matches C++ `CreateTranslationsAttr(VtValue const &defaultValue, bool writeSparsely)`.
    pub fn create_translations_attr(
        &self,
        default_value: Option<Value>,
        _write_sparsely: bool,
    ) -> Attribute {
        use usd_sdf::ValueTypeRegistry;

        let type_name = ValueTypeRegistry::instance().find_type("float3[]");
        if let Some(attr) =
            self.prim()
                .create_attribute(tokens().translations.as_str(), &type_name, false, None)
        {
            if let Some(val) = default_value {
                let _ = attr.set(val, TimeCode::default());
            }
            return attr;
        }

        Attribute::invalid()
    }

    // ========================================================================
    // ROTATIONS
    // ========================================================================

    /// Returns the rotations attribute.
    ///
    /// Joint-local unit quaternion rotations of all affected joints.
    ///
    /// Matches C++ `GetRotationsAttr()`.
    pub fn get_rotations_attr(&self) -> Attribute {
        self.prim()
            .get_attribute(tokens().rotations.as_str())
            .unwrap_or_else(Attribute::invalid)
    }

    /// Creates the rotations attribute.
    ///
    /// Matches C++ `CreateRotationsAttr(VtValue const &defaultValue, bool writeSparsely)`.
    pub fn create_rotations_attr(
        &self,
        default_value: Option<Value>,
        _write_sparsely: bool,
    ) -> Attribute {
        use usd_sdf::ValueTypeRegistry;

        let type_name = ValueTypeRegistry::instance().find_type("quatf[]");
        if let Some(attr) =
            self.prim()
                .create_attribute(tokens().rotations.as_str(), &type_name, false, None)
        {
            if let Some(val) = default_value {
                let _ = attr.set(val, TimeCode::default());
            }
            return attr;
        }

        Attribute::invalid()
    }

    // ========================================================================
    // SCALES
    // ========================================================================

    /// Returns the scales attribute.
    ///
    /// Joint-local scales of all affected joints.
    ///
    /// Matches C++ `GetScalesAttr()`.
    pub fn get_scales_attr(&self) -> Attribute {
        self.prim()
            .get_attribute(tokens().scales.as_str())
            .unwrap_or_else(Attribute::invalid)
    }

    /// Creates the scales attribute.
    ///
    /// Matches C++ `CreateScalesAttr(VtValue const &defaultValue, bool writeSparsely)`.
    pub fn create_scales_attr(
        &self,
        default_value: Option<Value>,
        _write_sparsely: bool,
    ) -> Attribute {
        use usd_sdf::ValueTypeRegistry;

        let type_name = ValueTypeRegistry::instance().find_type("half3[]");
        if let Some(attr) =
            self.prim()
                .create_attribute(tokens().scales.as_str(), &type_name, false, None)
        {
            if let Some(val) = default_value {
                let _ = attr.set(val, TimeCode::default());
            }
            return attr;
        }

        Attribute::invalid()
    }

    // ========================================================================
    // BLENDSHAPES
    // ========================================================================

    /// Returns the blendShapes attribute.
    ///
    /// Array of tokens identifying which blend shapes this
    /// animation's data applies to.
    ///
    /// Matches C++ `GetBlendShapesAttr()`.
    pub fn get_blend_shapes_attr(&self) -> Attribute {
        self.prim()
            .get_attribute(tokens().blend_shapes.as_str())
            .unwrap_or_else(Attribute::invalid)
    }

    /// Creates the blendShapes attribute.
    ///
    /// Matches C++ `CreateBlendShapesAttr(VtValue const &defaultValue, bool writeSparsely)`.
    pub fn create_blend_shapes_attr(
        &self,
        default_value: Option<Value>,
        _write_sparsely: bool,
    ) -> Attribute {
        use usd_sdf::ValueTypeRegistry;

        // blendShapes is uniform per C++ schema (uniform token[] blendShapes)
        let type_name = ValueTypeRegistry::instance().find_type("token[]");
        if let Some(attr) = self.prim().create_attribute(
            tokens().blend_shapes.as_str(),
            &type_name,
            false,
            Some(Variability::Uniform), // uniform per C++ schema
        ) {
            if let Some(val) = default_value {
                let _ = attr.set(val, TimeCode::default());
            }
            return attr;
        }

        Attribute::invalid()
    }

    // ========================================================================
    // BLENDSHAPEWEIGHTS
    // ========================================================================

    /// Returns the blendShapeWeights attribute.
    ///
    /// Array of weight values for each blend shape.
    ///
    /// Matches C++ `GetBlendShapeWeightsAttr()`.
    pub fn get_blend_shape_weights_attr(&self) -> Attribute {
        self.prim()
            .get_attribute(tokens().blend_shape_weights.as_str())
            .unwrap_or_else(Attribute::invalid)
    }

    /// Creates the blendShapeWeights attribute.
    ///
    /// Matches C++ `CreateBlendShapeWeightsAttr(VtValue const &defaultValue, bool writeSparsely)`.
    pub fn create_blend_shape_weights_attr(
        &self,
        default_value: Option<Value>,
        _write_sparsely: bool,
    ) -> Attribute {
        use usd_sdf::ValueTypeRegistry;

        let type_name = ValueTypeRegistry::instance().find_type("float[]");
        if let Some(attr) = self.prim().create_attribute(
            tokens().blend_shape_weights.as_str(),
            &type_name,
            false,
            None,
        ) {
            if let Some(val) = default_value {
                let _ = attr.set(val, TimeCode::default());
            }
            return attr;
        }

        Attribute::invalid()
    }

    // ========================================================================
    // CUSTOM CODE
    // ========================================================================

    /// Convenience method for querying resolved transforms at time.
    ///
    /// Note that it is more efficient to query transforms through
    /// UsdSkelAnimQuery or UsdSkelSkeletonQuery.
    ///
    /// Matches C++ `GetTransforms(VtMatrix4dArray* xforms, UsdTimeCode time)`.
    pub fn get_transforms(&self, time: TimeCode) -> Option<Vec<Matrix4d>> {
        use usd_gf::{Quatf, Vec3d, Vec3f, Vec3h};

        // Get translations, rotations, scales attributes
        let trans_attr = self.get_translations_attr();
        let rot_attr = self.get_rotations_attr();
        let scale_attr = self.get_scales_attr();

        if !trans_attr.is_valid() || !rot_attr.is_valid() || !scale_attr.is_valid() {
            return None;
        }

        // Get values at time
        let translations: Vec<Vec3f> = trans_attr.get_typed(time)?;
        let rotations: Vec<Quatf> = rot_attr.get_typed(time)?;
        let scales: Vec<Vec3h> = scale_attr.get_typed(time)?;

        // Compose transforms
        let count = translations.len().min(rotations.len()).min(scales.len());
        let mut xforms = Vec::with_capacity(count);

        for i in 0..count {
            let t = &translations[i];
            let r = &rotations[i];
            let s = &scales[i];

            // Build transform matrix from T, R, S components
            let xform = compose_transform(
                &Vec3d::new(t.x as f64, t.y as f64, t.z as f64),
                r,
                &Vec3d::new(f64::from(s.x), f64::from(s.y), f64::from(s.z)),
            );
            xforms.push(xform);
        }

        Some(xforms)
    }

    /// Convenience method for setting an array of transforms.
    /// The given transforms must be orthogonal.
    ///
    /// Decomposes each matrix into translation, rotation, and scale components
    /// and sets the corresponding attributes.
    ///
    /// Matches C++ `SetTransforms(const VtMatrix4dArray& xforms, UsdTimeCode time)`.
    pub fn set_transforms(&self, xforms: &[Matrix4d], time: TimeCode) -> bool {
        use usd_gf::{Quatf, Vec3f, Vec3h, half::Half};

        let mut translations = Vec::with_capacity(xforms.len());
        let mut rotations = Vec::with_capacity(xforms.len());
        let mut scales = Vec::with_capacity(xforms.len());

        for (i, xform) in xforms.iter().enumerate() {
            // Factor the matrix to extract scale, rotation, translation
            if let Some((scale_orient, scale, rotation, translation, _p)) = xform.factor() {
                // Extract translation as Vec3f
                translations.push(Vec3f::new(
                    translation.x as f32,
                    translation.y as f32,
                    translation.z as f32,
                ));

                // Extract rotation: combine rotation with scale orientation
                // then extract quaternion. For orthogonal matrices, scale_orient is identity.
                let rot_matrix = rotation * scale_orient.transpose();
                let quat = rot_matrix.extract_rotation_quat();
                rotations.push(Quatf::new(
                    quat.real() as f32,
                    usd_gf::Vec3f::new(
                        quat.imaginary().x as f32,
                        quat.imaginary().y as f32,
                        quat.imaginary().z as f32,
                    ),
                ));

                // Extract scale as Vec3h (half precision)
                scales.push(Vec3h::new(
                    Half::from(scale.x as f32),
                    Half::from(scale.y as f32),
                    Half::from(scale.z as f32),
                ));
            } else {
                // Failed to decompose - matrix may be singular
                eprintln!(
                    "Failed decomposing transform {}. The source transform may be singular.",
                    i
                );
                return false;
            }
        }

        // Set all three attributes
        let trans_ok = self
            .get_translations_attr()
            .set(Value::from(translations), time);
        let rot_ok = self.get_rotations_attr().set(Value::from(rotations), time);
        let scale_ok = self.get_scales_attr().set(Value::from(scales), time);

        trans_ok && rot_ok && scale_ok
    }
}

impl PartialEq for SkelAnimation {
    fn eq(&self, other: &Self) -> bool {
        self.prim() == other.prim()
    }
}

impl Eq for SkelAnimation {}

impl std::hash::Hash for SkelAnimation {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.prim().hash(state);
    }
}
