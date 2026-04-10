//! UsdGeomPointInstancer - point instancer schema.
//!
//! Port of pxr/usd/usdGeom/pointInstancer.h/cpp
//!
//! Encodes vectorized instancing of multiple, potentially animated, prototypes.

use super::boundable::Boundable;
use super::tokens::usd_geom_tokens;
use usd_core::attribute::Variability;
use usd_core::{Attribute, Prim, Relationship, Stage};
use usd_sdf::{TimeCode, ValueTypeRegistry};
use usd_tf::Token;
use usd_vt::Value;

use crate::schema_create_default::apply_optional_default;

// ============================================================================
// Value extraction helpers
// ============================================================================

/// Extract a typed array from a Value, handling both Array<T> and Vec<T> storage.
/// USDA parser stores as Vec<T>, USDC reader stores as Array<T>.
fn extract_array<T: Clone + Send + Sync + 'static>(value: &Value) -> Option<usd_vt::Array<T>> {
    if let Some(arr) = value.get::<usd_vt::Array<T>>() {
        return Some(arr.clone());
    }
    if let Some(vec) = value.get::<Vec<T>>() {
        return Some(vec.iter().cloned().collect());
    }
    None
}

#[derive(Clone)]
struct TransformSampleInfo<T: Clone + Send + Sync + 'static> {
    sample_time: TimeCode,
    lower_time_value: f64,
    upper_time_value: f64,
    has_samples: bool,
    values: usd_vt::Array<T>,
}

fn times_close(lhs: f64, rhs: f64) -> bool {
    (lhs - rhs).abs() <= f64::EPSILON
}

fn get_transform_sample<T: Clone + Send + Sync + 'static>(
    attr: &Attribute,
    base_time: TimeCode,
) -> Option<TransformSampleInfo<T>> {
    if base_time.is_default() {
        let value = attr.get(base_time)?;
        return Some(TransformSampleInfo {
            sample_time: base_time,
            lower_time_value: base_time.value(),
            upper_time_value: base_time.value(),
            has_samples: false,
            values: extract_array::<T>(&value)?,
        });
    }

    if let Some((lower, upper)) = attr.get_bracketing_time_samples(base_time.value()) {
        let value = attr.get(TimeCode::new(lower))?;
        let mut lower_time_value = lower;
        let mut upper_time_value = upper;
        if times_close(lower, upper) {
            let epsilon_time = base_time.value() + usd_core::TimeCode::safe_step(1e6, 10.0);
            if let Some((eps_lower, eps_upper)) = attr.get_bracketing_time_samples(epsilon_time) {
                lower_time_value = eps_lower;
                upper_time_value = eps_upper;
            }
        }
        return Some(TransformSampleInfo {
            sample_time: TimeCode::new(lower),
            lower_time_value,
            upper_time_value,
            has_samples: true,
            values: extract_array::<T>(&value)?,
        });
    }

    let value = attr.get(TimeCode::default_time())?;
    Some(TransformSampleInfo {
        sample_time: TimeCode::default_time(),
        lower_time_value: base_time.value(),
        upper_time_value: base_time.value(),
        has_samples: false,
        values: extract_array::<T>(&value)?,
    })
}

fn samples_are_aligned<T: Clone + Send + Sync + 'static, U: Clone + Send + Sync + 'static>(
    reference: &TransformSampleInfo<T>,
    candidate: &TransformSampleInfo<U>,
    expected_len: usize,
) -> bool {
    reference.has_samples
        && times_close(reference.lower_time_value, candidate.lower_time_value)
        && times_close(reference.upper_time_value, candidate.upper_time_value)
        && times_close(reference.sample_time.value(), candidate.sample_time.value())
        && candidate.values.len() == expected_len
}

fn get_inactive_ids(prim: &Prim) -> Vec<i64> {
    let Some(stage) = prim.stage() else {
        return Vec::new();
    };
    let Some(value) = stage.get_metadata_for_object(prim.path(), &usd_geom_tokens().inactive_ids)
    else {
        return Vec::new();
    };
    if let Some(list_op) = value.get::<usd_sdf::ListOp<i64>>() {
        return list_op.get_explicit_items().to_vec();
    }
    if let Some(items) = value.get::<Vec<i64>>() {
        return items.clone();
    }
    if let Some(items) = value.get::<usd_vt::Array<i64>>() {
        return items.iter().cloned().collect();
    }
    if let Some(list_op) = value.get::<usd_sdf::ListOp<i32>>() {
        return list_op
            .get_explicit_items()
            .iter()
            .map(|&item| item as i64)
            .collect();
    }
    if let Some(items) = value.get::<Vec<i32>>() {
        return items.iter().map(|&item| item as i64).collect();
    }
    if let Some(items) = value.get::<usd_vt::Array<i32>>() {
        return items.iter().map(|&item| item as i64).collect();
    }
    if let Some(items) = value.get::<Vec<Value>>() {
        return items
            .iter()
            .filter_map(|item| {
                item.get::<i64>()
                    .copied()
                    .or_else(|| item.get::<i32>().map(|v| i64::from(*v)))
            })
            .collect();
    }
    if let Some(items) = value.get::<usd_vt::Array<Value>>() {
        return items
            .iter()
            .filter_map(|item| {
                item.get::<i64>()
                    .copied()
                    .or_else(|| item.get::<i32>().map(|v| i64::from(*v)))
            })
            .collect();
    }
    Vec::new()
}

// ============================================================================
// Enums
// ============================================================================

/// Enum for prototype transform inclusion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProtoXformInclusion {
    /// Include the transform on the proto's root.
    IncludeProtoXform,
    /// Exclude the transform on the proto's root.
    ExcludeProtoXform,
}

/// Enum for mask application.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MaskApplication {
    /// Compute and apply the PointInstancer mask.
    ApplyMask,
    /// Ignore the PointInstancer mask.
    IgnoreMask,
}

impl std::fmt::Display for ProtoXformInclusion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProtoXformInclusion::IncludeProtoXform => write!(f, "IncludeProtoXform"),
            ProtoXformInclusion::ExcludeProtoXform => write!(f, "ExcludeProtoXform"),
        }
    }
}

impl std::fmt::Display for MaskApplication {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MaskApplication::ApplyMask => write!(f, "ApplyMask"),
            MaskApplication::IgnoreMask => write!(f, "IgnoreMask"),
        }
    }
}

// ============================================================================
// QuatToQuatd — trait to convert any supported quat type to Quatd
// ============================================================================

/// Convert a quaternion of any supported precision to `Quatd`.
///
/// Used to unify Quatf/Quath handling in generic instance-transform computation.
trait QuatToQuatd {
    fn to_quatd(&self) -> usd_gf::quat::Quatd;
}

impl QuatToQuatd for usd_gf::quat::Quatf {
    fn to_quatd(&self) -> usd_gf::quat::Quatd {
        use usd_gf::vec3::Vec3d;
        let i = self.imaginary();
        usd_gf::quat::Quatd::new(
            self.real() as f64,
            Vec3d::new(i.x as f64, i.y as f64, i.z as f64),
        )
    }
}

impl QuatToQuatd for usd_gf::quat::Quath {
    fn to_quatd(&self) -> usd_gf::quat::Quatd {
        use usd_gf::vec3::Vec3d;
        let i = self.imaginary();
        usd_gf::quat::Quatd::new(
            f64::from(self.real()),
            Vec3d::new(f64::from(i.x), f64::from(i.y), f64::from(i.z)),
        )
    }
}

// ============================================================================
// PointInstancer
// ============================================================================

/// Point instancer schema.
///
/// Encodes vectorized instancing of multiple, potentially animated, prototypes.
///
/// Matches C++ `UsdGeomPointInstancer`.
#[derive(Debug, Clone)]
pub struct PointInstancer {
    /// Base boundable schema.
    inner: Boundable,
}

impl PointInstancer {
    /// Creates a PointInstancer schema from a prim.
    ///
    /// Matches C++ `UsdGeomPointInstancer(const UsdPrim& prim)`.
    pub fn new(prim: Prim) -> Self {
        Self {
            inner: Boundable::new(prim),
        }
    }

    /// Creates a PointInstancer schema from a Boundable schema.
    ///
    /// Matches C++ `UsdGeomPointInstancer(const UsdSchemaBase& schemaObj)`.
    pub fn from_boundable(boundable: Boundable) -> Self {
        Self { inner: boundable }
    }

    /// Creates a PointInstancer schema from a Boundable reference.
    ///
    /// Convenience method for creating from a reference.
    pub fn from_boundable_ref(boundable: &Boundable) -> Self {
        Self {
            inner: boundable.clone(),
        }
    }

    /// Creates an invalid PointInstancer schema.
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
        Token::new("PointInstancer")
    }

    /// Return a PointInstancer holding the prim adhering to this schema at path on stage.
    ///
    /// Matches C++ `UsdGeomPointInstancer::Get(const UsdStagePtr &stage, const SdfPath &path)`.
    pub fn get(stage: &Stage, path: &usd_sdf::Path) -> Self {
        if let Some(prim) = stage.get_prim_at_path(path) {
            Self::new(prim)
        } else {
            Self::invalid()
        }
    }

    /// Attempt to ensure a prim adhering to this schema at path is defined on this stage.
    ///
    /// Matches C++ `UsdGeomPointInstancer::Define(const UsdStagePtr &stage, const SdfPath &path)`.
    pub fn define(stage: &Stage, path: &usd_sdf::Path) -> Self {
        match stage.define_prim(path.get_string(), Self::schema_type_name().as_str()) {
            Ok(prim) => Self::new(prim),
            Err(_) => Self::invalid(),
        }
    }

    // ========================================================================
    // ProtoIndices
    // ========================================================================

    /// Returns the protoIndices attribute.
    ///
    /// Per-instance index into prototypes relationship.
    ///
    /// Matches C++ `GetProtoIndicesAttr()`.
    pub fn get_proto_indices_attr(&self) -> Attribute {
        let prim = self.inner.prim();
        prim.get_attribute(usd_geom_tokens().proto_indices.as_str())
            .unwrap_or_else(Attribute::invalid)
    }

    /// Creates the protoIndices attribute.
    ///
    /// Matches C++ `CreateProtoIndicesAttr()`.
    pub fn create_proto_indices_attr(
        &self,
        default_value: Option<Value>,
        _write_sparsely: bool,
    ) -> Attribute {
        let prim = self.inner.prim();
        if !prim.is_valid() {
            return Attribute::invalid();
        }

        if prim.has_authored_attribute(usd_geom_tokens().proto_indices.as_str()) {
            return prim
                .get_attribute(usd_geom_tokens().proto_indices.as_str())
                .unwrap_or_else(Attribute::invalid);
        }

        let registry = ValueTypeRegistry::instance();
        let int_array_type = registry.find_type_by_token(&Token::new("int[]"));

        let attr = prim
            .create_attribute(
                usd_geom_tokens().proto_indices.as_str(),
                &int_array_type,
                false,                      // not custom
                Some(Variability::Varying), // can vary over time
            )
            .unwrap_or_else(Attribute::invalid);
        apply_optional_default(attr, default_value)
    }

    // ========================================================================
    // Ids
    // ========================================================================

    /// Returns the ids attribute.
    ///
    /// Optional 64-bit integer IDs for each instance.
    ///
    /// Matches C++ `GetIdsAttr()`.
    pub fn get_ids_attr(&self) -> Attribute {
        let prim = self.inner.prim();
        prim.get_attribute(usd_geom_tokens().ids.as_str())
            .unwrap_or_else(Attribute::invalid)
    }

    /// Creates the ids attribute.
    ///
    /// Matches C++ `CreateIdsAttr()`.
    pub fn create_ids_attr(
        &self,
        default_value: Option<Value>,
        _write_sparsely: bool,
    ) -> Attribute {
        let prim = self.inner.prim();
        if !prim.is_valid() {
            return Attribute::invalid();
        }

        if prim.has_authored_attribute(usd_geom_tokens().ids.as_str()) {
            return prim
                .get_attribute(usd_geom_tokens().ids.as_str())
                .unwrap_or_else(Attribute::invalid);
        }

        let registry = ValueTypeRegistry::instance();
        let int64_array_type = registry.find_type_by_token(&Token::new("int64[]"));

        let attr = prim
            .create_attribute(
                usd_geom_tokens().ids.as_str(),
                &int64_array_type,
                false,                      // not custom
                Some(Variability::Varying), // can vary over time
            )
            .unwrap_or_else(Attribute::invalid);
        apply_optional_default(attr, default_value)
    }

    // ========================================================================
    // Positions
    // ========================================================================

    /// Returns the positions attribute.
    ///
    /// Per-instance position.
    ///
    /// Matches C++ `GetPositionsAttr()`.
    pub fn get_positions_attr(&self) -> Attribute {
        let prim = self.inner.prim();
        prim.get_attribute(usd_geom_tokens().positions.as_str())
            .unwrap_or_else(Attribute::invalid)
    }

    /// Creates the positions attribute.
    ///
    /// Matches C++ `CreatePositionsAttr()`.
    pub fn create_positions_attr(
        &self,
        default_value: Option<Value>,
        _write_sparsely: bool,
    ) -> Attribute {
        let prim = self.inner.prim();
        if !prim.is_valid() {
            return Attribute::invalid();
        }

        if prim.has_authored_attribute(usd_geom_tokens().positions.as_str()) {
            return prim
                .get_attribute(usd_geom_tokens().positions.as_str())
                .unwrap_or_else(Attribute::invalid);
        }

        let registry = ValueTypeRegistry::instance();
        let point3f_array_type = registry.find_type_by_token(&Token::new("point3f[]"));

        let attr = prim
            .create_attribute(
                usd_geom_tokens().positions.as_str(),
                &point3f_array_type,
                false,                      // not custom
                Some(Variability::Varying), // can vary over time
            )
            .unwrap_or_else(Attribute::invalid);
        apply_optional_default(attr, default_value)
    }

    // ========================================================================
    // Orientations
    // ========================================================================

    /// Returns the orientations attribute (half precision quaternions).
    ///
    /// Per-instance orientation as unit quaternions.
    ///
    /// Matches C++ `GetOrientationsAttr()`.
    pub fn get_orientations_attr(&self) -> Attribute {
        let prim = self.inner.prim();
        prim.get_attribute(usd_geom_tokens().orientations.as_str())
            .unwrap_or_else(Attribute::invalid)
    }

    /// Creates the orientations attribute.
    ///
    /// Matches C++ `CreateOrientationsAttr()`.
    pub fn create_orientations_attr(
        &self,
        default_value: Option<Value>,
        _write_sparsely: bool,
    ) -> Attribute {
        let prim = self.inner.prim();
        if !prim.is_valid() {
            return Attribute::invalid();
        }

        if prim.has_authored_attribute(usd_geom_tokens().orientations.as_str()) {
            return prim
                .get_attribute(usd_geom_tokens().orientations.as_str())
                .unwrap_or_else(Attribute::invalid);
        }

        let registry = ValueTypeRegistry::instance();
        let quath_array_type = registry.find_type_by_token(&Token::new("quath[]"));

        let attr = prim
            .create_attribute(
                usd_geom_tokens().orientations.as_str(),
                &quath_array_type,
                false,                      // not custom
                Some(Variability::Varying), // can vary over time
            )
            .unwrap_or_else(Attribute::invalid);
        apply_optional_default(attr, default_value)
    }

    /// Returns the orientationsf attribute (full precision quaternions).
    ///
    /// Per-instance orientation as unit quaternions with full precision.
    ///
    /// Matches C++ `GetOrientationsfAttr()`.
    pub fn get_orientationsf_attr(&self) -> Attribute {
        let prim = self.inner.prim();
        prim.get_attribute(usd_geom_tokens().orientationsf.as_str())
            .unwrap_or_else(Attribute::invalid)
    }

    /// Creates the orientationsf attribute.
    ///
    /// Matches C++ `CreateOrientationsfAttr()`.
    pub fn create_orientationsf_attr(
        &self,
        default_value: Option<Value>,
        _write_sparsely: bool,
    ) -> Attribute {
        let prim = self.inner.prim();
        if !prim.is_valid() {
            return Attribute::invalid();
        }

        if prim.has_authored_attribute(usd_geom_tokens().orientationsf.as_str()) {
            return prim
                .get_attribute(usd_geom_tokens().orientationsf.as_str())
                .unwrap_or_else(Attribute::invalid);
        }

        let registry = ValueTypeRegistry::instance();
        let quatf_array_type = registry.find_type_by_token(&Token::new("quatf[]"));

        let attr = prim
            .create_attribute(
                usd_geom_tokens().orientationsf.as_str(),
                &quatf_array_type,
                false,                      // not custom
                Some(Variability::Varying), // can vary over time
            )
            .unwrap_or_else(Attribute::invalid);
        apply_optional_default(attr, default_value)
    }

    // ========================================================================
    // Scales
    // ========================================================================

    /// Returns the scales attribute.
    ///
    /// Per-instance scale to be applied before rotation.
    ///
    /// Matches C++ `GetScalesAttr()`.
    pub fn get_scales_attr(&self) -> Attribute {
        let prim = self.inner.prim();
        prim.get_attribute(usd_geom_tokens().scales.as_str())
            .unwrap_or_else(Attribute::invalid)
    }

    /// Creates the scales attribute.
    ///
    /// Matches C++ `CreateScalesAttr()`.
    pub fn create_scales_attr(
        &self,
        default_value: Option<Value>,
        _write_sparsely: bool,
    ) -> Attribute {
        let prim = self.inner.prim();
        if !prim.is_valid() {
            return Attribute::invalid();
        }

        if prim.has_authored_attribute(usd_geom_tokens().scales.as_str()) {
            return prim
                .get_attribute(usd_geom_tokens().scales.as_str())
                .unwrap_or_else(Attribute::invalid);
        }

        let registry = ValueTypeRegistry::instance();
        let float3_array_type = registry.find_type_by_token(&Token::new("float3[]"));

        let attr = prim
            .create_attribute(
                usd_geom_tokens().scales.as_str(),
                &float3_array_type,
                false,                      // not custom
                Some(Variability::Varying), // can vary over time
            )
            .unwrap_or_else(Attribute::invalid);
        apply_optional_default(attr, default_value)
    }

    // ========================================================================
    // Velocities
    // ========================================================================

    /// Returns the velocities attribute.
    ///
    /// Per-instance velocities for motion blur.
    ///
    /// Matches C++ `GetVelocitiesAttr()`.
    pub fn get_velocities_attr(&self) -> Attribute {
        let prim = self.inner.prim();
        prim.get_attribute(usd_geom_tokens().velocities.as_str())
            .unwrap_or_else(Attribute::invalid)
    }

    /// Creates the velocities attribute.
    ///
    /// Matches C++ `CreateVelocitiesAttr()`.
    pub fn create_velocities_attr(
        &self,
        default_value: Option<Value>,
        _write_sparsely: bool,
    ) -> Attribute {
        let prim = self.inner.prim();
        if !prim.is_valid() {
            return Attribute::invalid();
        }

        if prim.has_authored_attribute(usd_geom_tokens().velocities.as_str()) {
            return prim
                .get_attribute(usd_geom_tokens().velocities.as_str())
                .unwrap_or_else(Attribute::invalid);
        }

        let registry = ValueTypeRegistry::instance();
        let vector3f_array_type = registry.find_type_by_token(&Token::new("vector3f[]"));

        let attr = prim
            .create_attribute(
                usd_geom_tokens().velocities.as_str(),
                &vector3f_array_type,
                false,                      // not custom
                Some(Variability::Varying), // can vary over time
            )
            .unwrap_or_else(Attribute::invalid);
        apply_optional_default(attr, default_value)
    }

    // ========================================================================
    // Accelerations
    // ========================================================================

    /// Returns the accelerations attribute.
    ///
    /// Per-instance accelerations for motion blur.
    ///
    /// Matches C++ `GetAccelerationsAttr()`.
    pub fn get_accelerations_attr(&self) -> Attribute {
        let prim = self.inner.prim();
        prim.get_attribute(usd_geom_tokens().accelerations.as_str())
            .unwrap_or_else(Attribute::invalid)
    }

    /// Creates the accelerations attribute.
    ///
    /// Matches C++ `CreateAccelerationsAttr()`.
    pub fn create_accelerations_attr(
        &self,
        default_value: Option<Value>,
        _write_sparsely: bool,
    ) -> Attribute {
        let prim = self.inner.prim();
        if !prim.is_valid() {
            return Attribute::invalid();
        }

        if prim.has_authored_attribute(usd_geom_tokens().accelerations.as_str()) {
            return prim
                .get_attribute(usd_geom_tokens().accelerations.as_str())
                .unwrap_or_else(Attribute::invalid);
        }

        let registry = ValueTypeRegistry::instance();
        let vector3f_array_type = registry.find_type_by_token(&Token::new("vector3f[]"));

        let attr = prim
            .create_attribute(
                usd_geom_tokens().accelerations.as_str(),
                &vector3f_array_type,
                false,                      // not custom
                Some(Variability::Varying), // can vary over time
            )
            .unwrap_or_else(Attribute::invalid);
        apply_optional_default(attr, default_value)
    }

    // ========================================================================
    // AngularVelocities
    // ========================================================================

    /// Returns the angularVelocities attribute.
    ///
    /// Per-instance angular velocities for motion blur.
    ///
    /// Matches C++ `GetAngularVelocitiesAttr()`.
    pub fn get_angular_velocities_attr(&self) -> Attribute {
        let prim = self.inner.prim();
        prim.get_attribute(usd_geom_tokens().angular_velocities.as_str())
            .unwrap_or_else(Attribute::invalid)
    }

    /// Creates the angularVelocities attribute.
    ///
    /// Matches C++ `CreateAngularVelocitiesAttr()`.
    pub fn create_angular_velocities_attr(
        &self,
        default_value: Option<Value>,
        _write_sparsely: bool,
    ) -> Attribute {
        let prim = self.inner.prim();
        if !prim.is_valid() {
            return Attribute::invalid();
        }

        if prim.has_authored_attribute(usd_geom_tokens().angular_velocities.as_str()) {
            return prim
                .get_attribute(usd_geom_tokens().angular_velocities.as_str())
                .unwrap_or_else(Attribute::invalid);
        }

        let registry = ValueTypeRegistry::instance();
        let vector3f_array_type = registry.find_type_by_token(&Token::new("vector3f[]"));

        let attr = prim
            .create_attribute(
                usd_geom_tokens().angular_velocities.as_str(),
                &vector3f_array_type,
                false,                      // not custom
                Some(Variability::Varying), // can vary over time
            )
            .unwrap_or_else(Attribute::invalid);
        apply_optional_default(attr, default_value)
    }

    // ========================================================================
    // InvisibleIds
    // ========================================================================

    /// Returns the invisibleIds attribute.
    ///
    /// A list of IDs to make invisible at the evaluation time.
    ///
    /// Matches C++ `GetInvisibleIdsAttr()`.
    pub fn get_invisible_ids_attr(&self) -> Attribute {
        let prim = self.inner.prim();
        prim.get_attribute(usd_geom_tokens().invisible_ids.as_str())
            .unwrap_or_else(Attribute::invalid)
    }

    /// Creates the invisibleIds attribute.
    ///
    /// Matches C++ `CreateInvisibleIdsAttr()`.
    pub fn create_invisible_ids_attr(
        &self,
        default_value: Option<Value>,
        _write_sparsely: bool,
    ) -> Attribute {
        let prim = self.inner.prim();
        if !prim.is_valid() {
            return Attribute::invalid();
        }

        if prim.has_authored_attribute(usd_geom_tokens().invisible_ids.as_str()) {
            return prim
                .get_attribute(usd_geom_tokens().invisible_ids.as_str())
                .unwrap_or_else(Attribute::invalid);
        }

        let registry = ValueTypeRegistry::instance();
        let int64_array_type = registry.find_type_by_token(&Token::new("int64[]"));

        let attr = prim
            .create_attribute(
                usd_geom_tokens().invisible_ids.as_str(),
                &int64_array_type,
                false,                      // not custom
                Some(Variability::Varying), // can vary over time
            )
            .unwrap_or_else(Attribute::invalid);
        apply_optional_default(attr, default_value)
    }

    // ========================================================================
    // Prototypes
    // ========================================================================

    /// Returns the prototypes relationship.
    ///
    /// Orders and targets the prototype root prims.
    ///
    /// Matches C++ `GetPrototypesRel()`.
    pub fn get_prototypes_rel(&self) -> Relationship {
        let prim = self.inner.prim();
        prim.get_relationship(usd_geom_tokens().prototypes.as_str())
            .unwrap_or_else(Relationship::invalid)
    }

    /// Creates the prototypes relationship.
    ///
    /// Matches C++ `CreatePrototypesRel()`.
    pub fn create_prototypes_rel(&self) -> Relationship {
        let prim = self.inner.prim();
        if !prim.is_valid() {
            return Relationship::invalid();
        }

        if let Some(rel) = prim.get_relationship(usd_geom_tokens().prototypes.as_str()) {
            return rel;
        }

        prim.create_relationship(usd_geom_tokens().prototypes.as_str(), false)
            .unwrap_or_else(Relationship::invalid)
    }

    /// Return a vector of names of all pre-declared attributes for this schema
    /// class and all its ancestor classes.
    ///
    /// Matches C++ `GetSchemaAttributeNames(bool includeInherited)`.
    pub fn get_schema_attribute_names(include_inherited: bool) -> Vec<Token> {
        let local_names = vec![
            usd_geom_tokens().proto_indices.clone(),
            usd_geom_tokens().ids.clone(),
            usd_geom_tokens().positions.clone(),
            usd_geom_tokens().orientations.clone(),
            usd_geom_tokens().orientationsf.clone(),
            usd_geom_tokens().scales.clone(),
            usd_geom_tokens().velocities.clone(),
            usd_geom_tokens().accelerations.clone(),
            usd_geom_tokens().angular_velocities.clone(),
            usd_geom_tokens().invisible_ids.clone(),
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
    // Id-based Instance Masking/Pruning
    // ========================================================================

    /// Helper to set or merge over list op for inactiveIds.
    fn set_or_merge_over_op(&self, items: Vec<i64>, op_type: usd_sdf::ListOpType) -> bool {
        use usd_sdf::ListOp;
        use usd_vt::Value;

        let prim = self.inner.prim();
        if !prim.is_valid() {
            return false;
        }

        // Get current inactiveIds metadata
        let current_op: ListOp<i64> = prim
            .get_metadata(&usd_geom_tokens().inactive_ids)
            .unwrap_or_default();

        // Create proposed op
        let mut proposed_op = ListOp::new();
        let _ = proposed_op.set_items(items, op_type);

        // Apply operations
        let mut current_items = current_op.get_applied_items();
        proposed_op.apply_operations(
            &mut current_items,
            None::<fn(usd_sdf::ListOpType, &i64) -> Option<i64>>,
        );

        // Set as explicit
        let result_op = ListOp::create_explicit(current_items);
        prim.set_metadata(
            &usd_geom_tokens().inactive_ids,
            Value::from_no_hash(result_op),
        )
    }

    /// Ensure that the instance identified by id is active over all time.
    ///
    /// Matches C++ `ActivateId()`.
    pub fn activate_id(&self, id: i64) -> bool {
        let to_remove = vec![id];
        self.set_or_merge_over_op(to_remove, usd_sdf::ListOpType::Deleted)
    }

    /// Ensure that the instances identified by ids are active over all time.
    ///
    /// Matches C++ `ActivateIds()`.
    pub fn activate_ids(&self, ids: &[i64]) -> bool {
        let to_remove = ids.to_vec();
        self.set_or_merge_over_op(to_remove, usd_sdf::ListOpType::Deleted)
    }

    /// Ensure that all instances are active over all time.
    ///
    /// Matches C++ `ActivateAllIds()`.
    pub fn activate_all_ids(&self) -> bool {
        use usd_sdf::ListOp;

        let prim = self.inner.prim();
        if !prim.is_valid() {
            return false;
        }
        let op = ListOp::create_explicit(Vec::<i64>::new());
        prim.set_metadata(&usd_geom_tokens().inactive_ids, Value::from_no_hash(op))
    }

    /// Ensure that the instance identified by id is inactive over all time.
    ///
    /// Matches C++ `DeactivateId()`.
    pub fn deactivate_id(&self, id: i64) -> bool {
        let to_add = vec![id];
        self.set_or_merge_over_op(to_add, usd_sdf::ListOpType::Appended)
    }

    /// Ensure that the instances identified by ids are inactive over all time.
    ///
    /// Matches C++ `DeactivateIds()`.
    pub fn deactivate_ids(&self, ids: &[i64]) -> bool {
        let to_add = ids.to_vec();
        self.set_or_merge_over_op(to_add, usd_sdf::ListOpType::Appended)
    }

    /// Ensure that the instance identified by id is visible at time.
    ///
    /// Matches C++ `VisId()`.
    pub fn vis_id(&self, id: i64, time: TimeCode) -> bool {
        self.vis_ids(&[id], time)
    }

    /// Ensure that the instances identified by ids are visible at time.
    ///
    /// Matches C++ `VisIds()`.
    pub fn vis_ids(&self, ids: &[i64], time: TimeCode) -> bool {
        use usd_vt::Array;

        let invisible_ids_attr = self.get_invisible_ids_attr();
        if !invisible_ids_attr.is_valid() {
            return true;
        }

        let mut invised: Array<i64> = Array::new();
        if let Some(value) = invisible_ids_attr.get(time) {
            if let Some(arr) = value.get::<Array<i64>>() {
                invised = arr.clone();
            } else if let Some(vec) = value.get::<Vec<i64>>() {
                invised = vec.iter().cloned().collect();
            }
        }

        let mut invis_set: std::collections::HashSet<i64> = invised.iter().cloned().collect();
        let mut num_removed = 0;

        for &id in ids {
            if invis_set.remove(&id) {
                num_removed += 1;
            }
        }

        if num_removed > 0 {
            let new_invised: Vec<i64> = invis_set.into_iter().collect();
            use usd_vt::Array;
            let arr: Array<i64> = new_invised.iter().cloned().collect();
            invisible_ids_attr.set(usd_vt::Value::from_no_hash(arr), time)
        } else {
            true
        }
    }

    /// Ensure that all instances are visible at time.
    ///
    /// Matches C++ `VisAllIds()`.
    pub fn vis_all_ids(&self, time: TimeCode) -> bool {
        let invisible_ids_attr = self.create_invisible_ids_attr(None, false);
        if invisible_ids_attr.has_value() {
            use usd_vt::Array;
            let empty: Array<i64> = Array::new();
            invisible_ids_attr.set(usd_vt::Value::from_no_hash(empty), time)
        } else {
            true
        }
    }

    /// Ensure that the instance identified by id is invisible at time.
    ///
    /// Matches C++ `InvisId()`.
    pub fn invis_id(&self, id: i64, time: TimeCode) -> bool {
        self.invis_ids(&[id], time)
    }

    /// Ensure that the instances identified by ids are invisible at time.
    ///
    /// Matches C++ `InvisIds()`.
    pub fn invis_ids(&self, ids: &[i64], time: TimeCode) -> bool {
        use usd_vt::Array;

        let invisible_ids_attr = self.get_invisible_ids_attr();
        let mut invised: Array<i64> = Array::new();

        if invisible_ids_attr.is_valid() {
            if let Some(value) = invisible_ids_attr.get(time) {
                if let Some(arr) = value.get::<Array<i64>>() {
                    invised = arr.clone();
                } else if let Some(vec) = value.get::<Vec<i64>>() {
                    invised = vec.iter().cloned().collect();
                }
            }
        }

        let mut invis_set: std::collections::HashSet<i64> = invised.iter().cloned().collect();
        let mut changed = false;

        for &id in ids {
            if !invis_set.contains(&id) {
                invised.push(id);
                invis_set.insert(id);
                changed = true;
            }
        }

        if changed {
            let new_invised: Vec<i64> = invised.iter().cloned().collect();
            use usd_vt::Array;
            let arr: Array<i64> = new_invised.iter().cloned().collect();
            self.create_invisible_ids_attr(None, false)
                .set(usd_vt::Value::from_no_hash(arr), time)
        } else {
            true
        }
    }

    /// Computes a presence mask to be applied to per-instance data arrays.
    ///
    /// Matches C++ `ComputeMaskAtTime()`.
    pub fn compute_mask_at_time(&self, time: TimeCode, ids: Option<&[i64]>) -> Vec<bool> {
        use usd_vt::Array;

        let mut id_vals: Vec<i64> = Vec::new();
        let mut invised_ids: Array<i64> = Array::new();
        let mut mask: Vec<bool> = Vec::new();

        // Get inactiveIds metadata
        let prim = self.inner.prim();
        let inactive_ids = get_inactive_ids(prim);

        // Get invisibleIds attribute
        if let Some(value) = self.get_invisible_ids_attr().get(time) {
            if let Some(arr) = value.get::<Array<i64>>() {
                invised_ids = arr.clone();
            } else if let Some(vec) = value.get::<Vec<i64>>() {
                invised_ids = vec.iter().cloned().collect();
            }
        }

        if !inactive_ids.is_empty() || !invised_ids.is_empty() {
            let mut masked_ids: std::collections::HashSet<i64> =
                inactive_ids.iter().cloned().collect();
            masked_ids.extend(invised_ids.iter().cloned());

            let ids_to_check: &[i64] = if let Some(provided_ids) = ids {
                provided_ids
            } else {
                // Try to get from ids attribute
                if let Some(value) = self.get_ids_attr().get(time) {
                    if let Some(arr) = value.get::<Array<i64>>() {
                        id_vals = arr.iter().cloned().collect();
                    } else if let Some(vec) = value.get::<Vec<i64>>() {
                        id_vals = vec.clone();
                    }
                }

                // Fallback to protoIndices indices
                if id_vals.is_empty() {
                    if let Some(value) = self.get_proto_indices_attr().get(time) {
                        if let Some(arr) = value.get::<Array<i32>>() {
                            let num_instances = arr.len();
                            id_vals = (0..num_instances as i64).collect();
                        } else if let Some(vec) = value.get::<Vec<i32>>() {
                            let num_instances = vec.len();
                            id_vals = (0..num_instances as i64).collect();
                        }
                    }
                }

                if !id_vals.is_empty() {
                    &id_vals
                } else {
                    return mask; // Empty mask
                }
            };

            let mut any_pruned = false;
            mask.reserve(ids_to_check.len());
            for &id in ids_to_check {
                let pruned = masked_ids.contains(&id);
                any_pruned = any_pruned || pruned;
                mask.push(!pruned);
            }

            // If no instances are pruned, return empty mask
            if !any_pruned {
                mask.clear();
            }
        }

        mask
    }

    /// Contract dataArray in-place to contain only the elements whose index in mask is true.
    ///
    /// Matches C++ `ApplyMaskToArray()`.
    pub fn apply_mask_to_array<T: Clone>(
        mask: &[bool],
        data_array: &mut Vec<T>,
        element_size: usize,
    ) -> bool {
        if mask.is_empty() {
            // Empty mask means "all pass"
            return true;
        }

        if data_array.is_empty() {
            return true;
        }

        let expected_size = element_size * mask.len();
        if data_array.len() != expected_size {
            // Warning: size mismatch
            return false;
        }

        let mut new_array = Vec::new();
        for (i, &pass) in mask.iter().enumerate() {
            if pass {
                let start = i * element_size;
                let end = start + element_size;
                if end <= data_array.len() {
                    new_array.extend_from_slice(&data_array[start..end]);
                }
            }
        }

        *data_array = new_array;
        true
    }

    /// Determines if we should prefer orientationsf over orientations.
    ///
    /// Matches C++ `UsesOrientationsf()`.
    pub fn uses_orientationsf(&self) -> (bool, Attribute) {
        let orientationsf_attr = self.get_orientationsf_attr();
        if orientationsf_attr.is_valid() {
            // Check default time first, then try to find earliest time sample
            let default_time = TimeCode::default_time();
            if let Some(value) = orientationsf_attr.get(default_time) {
                if let Some(arr) = value.get::<usd_vt::Array<usd_gf::quat::Quatf>>() {
                    if !arr.is_empty() {
                        return (true, orientationsf_attr);
                    }
                } else if let Some(vec) = value.get::<Vec<usd_gf::quat::Quatf>>() {
                    if !vec.is_empty() {
                        return (true, orientationsf_attr);
                    }
                }
            }

            // Try to get earliest time sample
            if let Some(time_samples) = orientationsf_attr.get_time_samples().first().copied() {
                let earliest_time = TimeCode::new(time_samples);
                if let Some(value) = orientationsf_attr.get(earliest_time) {
                    if let Some(arr) = value.get::<usd_vt::Array<usd_gf::quat::Quatf>>() {
                        if !arr.is_empty() {
                            return (true, orientationsf_attr);
                        }
                    } else if let Some(vec) = value.get::<Vec<usd_gf::quat::Quatf>>() {
                        if !vec.is_empty() {
                            return (true, orientationsf_attr);
                        }
                    }
                }
            }
        }

        (false, self.get_orientations_attr())
    }

    // ========================================================================
    // Instance Transform Computation Helpers
    // ========================================================================

    /// Helper to get proto indices for instance transforms.
    ///
    /// Matches C++ `_GetProtoIndicesForInstanceTransforms()`.
    fn get_proto_indices_for_instance_transforms(
        &self,
        base_time: TimeCode,
    ) -> Option<usd_vt::Array<i32>> {
        use usd_vt::Array;

        // Try to extract int array from a Value (handles both Array<i32> and Vec<i32>)
        fn extract_int_array(v: &usd_vt::Value) -> Option<Array<i32>> {
            if let Some(arr) = v.get::<Array<i32>>() {
                return Some(arr.clone());
            }
            if let Some(vec) = v.get::<Vec<i32>>() {
                return Some(vec.iter().copied().collect());
            }
            None
        }

        let proto_indices_attr = self.get_proto_indices_attr();
        if !proto_indices_attr.is_valid() {
            return None;
        }

        if base_time.is_default() {
            // baseTime is default - just get at default time
            proto_indices_attr
                .get(base_time)
                .and_then(|v| extract_int_array(&v))
        } else {
            // baseTime is numeric - get bracketing time samples
            let time_value = base_time.value();
            if let Some((lower, _upper)) =
                proto_indices_attr.get_bracketing_time_samples(time_value)
            {
                let sample_time = TimeCode::new(lower);
                proto_indices_attr
                    .get(sample_time)
                    .and_then(|v| extract_int_array(&v))
            } else {
                // No samples - try default
                proto_indices_attr
                    .get(TimeCode::default_time())
                    .and_then(|v| extract_int_array(&v))
            }
        }
    }

    /// Helper to get prototype paths for instance transforms.
    ///
    /// Matches C++ `_GetPrototypePathsForInstanceTransforms()`.
    fn get_prototype_paths_for_instance_transforms(
        &self,
        proto_indices: &usd_vt::Array<i32>,
    ) -> Option<Vec<usd_sdf::Path>> {
        let prototypes_rel = self.get_prototypes_rel();
        if !prototypes_rel.is_valid() {
            return None;
        }

        let proto_paths = prototypes_rel.get_targets();
        if proto_paths.is_empty() {
            return None;
        }

        // Validate all proto indices are in bounds
        for &proto_index in proto_indices.iter() {
            if proto_index < 0 || proto_index as usize >= proto_paths.len() {
                return None;
            }
        }

        Some(proto_paths)
    }

    /// Helper to compute point instancer attributes preamble.
    ///
    /// Matches C++ `_ComputePointInstancerAttributesPreamble()`.
    fn compute_point_instancer_attributes_preamble(
        &self,
        base_time: TimeCode,
        do_proto_xforms: ProtoXformInclusion,
        apply_mask: MaskApplication,
    ) -> Option<(usd_vt::Array<i32>, Vec<usd_sdf::Path>, Vec<bool>)> {
        let proto_indices = self.get_proto_indices_for_instance_transforms(base_time)?;
        let num_instances = proto_indices.len();

        let proto_paths = if do_proto_xforms == ProtoXformInclusion::IncludeProtoXform {
            self.get_prototype_paths_for_instance_transforms(&proto_indices)?
        } else {
            Vec::new()
        };

        let mask = if apply_mask == MaskApplication::ApplyMask {
            let computed_mask = self.compute_mask_at_time(base_time, None);
            if !computed_mask.is_empty() && computed_mask.len() != num_instances {
                return None; // Mask size mismatch
            }
            computed_mask
        } else {
            Vec::new()
        };

        Some((proto_indices, proto_paths, mask))
    }

    /// Helper to calculate time delta between two time codes.
    ///
    /// Matches C++ `UsdGeom_CalculateTimeDelta()`.
    fn calculate_time_delta(
        time: TimeCode,
        sample_time: TimeCode,
        time_codes_per_second: f64,
    ) -> f64 {
        if time.is_default() || sample_time.is_default() {
            return 0.0;
        }
        (time.value() - sample_time.value()) / time_codes_per_second
    }

    /// Generic helper: compute per-instance transforms at a single time.
    ///
    /// Unifies C++ `_DoComputeInstanceTransformsAtTime<GfQuatf>()` and
    /// `_DoComputeInstanceTransformsAtTime<GfQuath>()` via the `QuatToQuatd` trait.
    fn do_compute_xforms_at_time<Q: QuatToQuatd + Clone + Send + Sync + 'static>(
        &self,
        xforms: &mut Vec<usd_gf::matrix4::Matrix4d>,
        stage: &usd_core::Stage,
        time: TimeCode,
        proto_indices: &usd_vt::Array<i32>,
        positions: &usd_vt::Array<usd_gf::vec3::Vec3f>,
        velocities: &usd_vt::Array<usd_gf::vec3::Vec3f>,
        velocities_sample_time: TimeCode,
        accelerations: &usd_vt::Array<usd_gf::vec3::Vec3f>,
        scales: &usd_vt::Array<usd_gf::vec3::Vec3f>,
        orientations: &usd_vt::Array<Q>,
        angular_velocities: &usd_vt::Array<usd_gf::vec3::Vec3f>,
        angular_velocities_sample_time: TimeCode,
        proto_paths: &[usd_sdf::Path],
        mask: &[bool],
    ) -> bool {
        Self::compute_xforms_at_time_impl(
            xforms,
            stage,
            time,
            proto_indices,
            positions,
            velocities,
            velocities_sample_time,
            accelerations,
            scales,
            orientations,
            angular_velocities,
            angular_velocities_sample_time,
            proto_paths,
            mask,
            Self::calculate_time_delta,
        )
    }

    /// Inner implementation shared by instance and static paths.
    ///
    /// Accepts a `time_delta_fn` so the instance method and the static method
    /// can each supply their own (identical) time-delta calculation without
    /// needing `&self` in the static path.
    fn compute_xforms_at_time_impl<Q: QuatToQuatd + Clone + Send + Sync + 'static, F>(
        xforms: &mut Vec<usd_gf::matrix4::Matrix4d>,
        stage: &usd_core::Stage,
        time: TimeCode,
        proto_indices: &usd_vt::Array<i32>,
        positions: &usd_vt::Array<usd_gf::vec3::Vec3f>,
        velocities: &usd_vt::Array<usd_gf::vec3::Vec3f>,
        velocities_sample_time: TimeCode,
        accelerations: &usd_vt::Array<usd_gf::vec3::Vec3f>,
        scales: &usd_vt::Array<usd_gf::vec3::Vec3f>,
        orientations: &usd_vt::Array<Q>,
        angular_velocities: &usd_vt::Array<usd_gf::vec3::Vec3f>,
        angular_velocities_sample_time: TimeCode,
        proto_paths: &[usd_sdf::Path],
        mask: &[bool],
        time_delta_fn: F,
    ) -> bool
    where
        F: Fn(TimeCode, TimeCode, f64) -> f64,
    {
        use super::xform_cache::XformCache;
        use usd_gf::matrix4::Matrix4d;
        use usd_gf::vec3::Vec3f;

        let num_instances = proto_indices.len();
        xforms.resize(num_instances, Matrix4d::identity());

        let time_codes_per_second = stage.get_time_codes_per_second();
        let velocity_time_delta =
            time_delta_fn(time, velocities_sample_time, time_codes_per_second);
        let angular_velocity_time_delta =
            time_delta_fn(time, angular_velocities_sample_time, time_codes_per_second);

        // Get prototype transforms
        let mut proto_xforms = vec![Matrix4d::identity(); proto_paths.len()];
        if !proto_paths.is_empty() {
            // debug removed
            let mut xform_cache = XformCache::new(time);
            for (proto_index, proto_path) in proto_paths.iter().enumerate() {
                if let Some(proto_prim) = stage.get_prim_at_path(proto_path) {
                    let (local_xform, _resets_xform_stack) =
                        xform_cache.get_local_transformation(&proto_prim);
                    proto_xforms[proto_index] = local_xform;
                    // debug removed
                }
            }
        }

        // Compute per-instance transforms
        for instance_id in 0..num_instances {
            if !mask.is_empty() && !mask[instance_id] {
                continue;
            }

            let mut instance_transform = Matrix4d::identity();
            let mut is_identity = true;

            // Apply scale
            if !scales.is_empty() && instance_id < scales.len() {
                let scale = &scales[instance_id];
                let scale_vec =
                    usd_gf::vec3::Vec3d::new(scale.x as f64, scale.y as f64, scale.z as f64);
                instance_transform.set_scale_vec(&scale_vec);
                is_identity = false;
            }

            // Apply rotation — convert any Quat precision to Quatd via trait
            if !orientations.is_empty() && instance_id < orientations.len() {
                let quatd = orientations[instance_id].to_quatd();
                let mut rot_matrix = Matrix4d::identity();
                rot_matrix.set_rotate(&quatd);

                if is_identity {
                    instance_transform = rot_matrix;
                } else {
                    instance_transform *= rot_matrix;
                }

                // Apply angular velocity
                if !angular_velocities.is_empty() && instance_id < angular_velocities.len() {
                    let angular_velocity = &angular_velocities[instance_id];
                    // C++ uses GfRotation(angularVelocity, delta * length)
                    // where angular velocity is in degrees/sec and GfRotation
                    // takes (axis, angleDegrees). So delta*length = degrees.
                    let angular_vel_length = angular_velocity.length();
                    let angle_degrees = angular_velocity_time_delta as f32 * angular_vel_length;
                    if angle_degrees.abs() > 1e-6 {
                        use usd_gf::rotation::Rotation;
                        use usd_gf::vec3::Vec3d;
                        let axis = Vec3d::new(
                            angular_velocity.x as f64,
                            angular_velocity.y as f64,
                            angular_velocity.z as f64,
                        );
                        let rotation = Rotation::from_axis_angle(axis, angle_degrees as f64);
                        let mut rot_matrix = Matrix4d::identity();
                        rot_matrix.set_rotate(&rotation.get_quat());
                        instance_transform *= rot_matrix;
                    }
                }
            }

            // Apply translation with optional velocity integration
            let mut translation = if instance_id < positions.len() {
                positions[instance_id]
            } else {
                Vec3f::new(0.0, 0.0, 0.0)
            };

            if !velocities.is_empty() && instance_id < velocities.len() {
                let mut velocity = velocities[instance_id];
                if !accelerations.is_empty() && instance_id < accelerations.len() {
                    let accel = accelerations[instance_id];
                    velocity.x += velocity_time_delta as f32 * accel.x * 0.5;
                    velocity.y += velocity_time_delta as f32 * accel.y * 0.5;
                    velocity.z += velocity_time_delta as f32 * accel.z * 0.5;
                }
                translation.x += velocity_time_delta as f32 * velocity.x;
                translation.y += velocity_time_delta as f32 * velocity.y;
                translation.z += velocity_time_delta as f32 * velocity.z;
            }

            instance_transform.set_translate_only(&usd_gf::vec3::Vec3d::new(
                translation.x as f64,
                translation.y as f64,
                translation.z as f64,
            ));

            // Compose with prototype transform
            let proto_index = proto_indices[instance_id];
            if !proto_paths.is_empty()
                && proto_index >= 0
                && (proto_index as usize) < proto_xforms.len()
            {
                xforms[instance_id] = proto_xforms[proto_index as usize] * instance_transform;
            } else {
                xforms[instance_id] = instance_transform;
            }

            if instance_id == 0 && num_instances == 1 {
                // debug removed
            }
        }

        // Apply mask
        Self::apply_mask_to_array(mask, xforms, 1)
    }

    // ========================================================================
    // Public Instance Transform Computation Methods
    // ========================================================================

    /// Compute the per-instance transforms at the given time.
    ///
    /// Matches C++ `ComputeInstanceTransformsAtTime()`.
    pub fn compute_instance_transforms_at_time(
        &self,
        xforms: &mut Vec<usd_gf::matrix4::Matrix4d>,
        time: TimeCode,
        base_time: TimeCode,
        do_proto_xforms: ProtoXformInclusion,
        apply_mask: MaskApplication,
    ) -> bool {
        let (proto_indices, proto_paths, mask) = match self
            .compute_point_instancer_attributes_preamble(base_time, do_proto_xforms, apply_mask)
        {
            Some(result) => result,
            None => return false,
        };

        let stage = match self.prim().stage() {
            Some(s) => s,
            None => return false,
        };

        use usd_vt::Array;

        let positions_attr = self.get_positions_attr();
        if !positions_attr.is_valid() {
            return false;
        }

        let positions_info =
            match get_transform_sample::<usd_gf::vec3::Vec3f>(&positions_attr, base_time) {
                Some(info) => info,
                None => return false,
            };

        let mut velocities: Array<usd_gf::vec3::Vec3f> = Array::new();
        let mut accelerations: Array<usd_gf::vec3::Vec3f> = Array::new();
        let mut velocities_sample_time = TimeCode::default_time();
        let mut velocities_info = None;

        let velocities_attr = self.get_velocities_attr();
        if velocities_attr.is_valid() {
            if let Some(candidate) =
                get_transform_sample::<usd_gf::vec3::Vec3f>(&velocities_attr, base_time)
            {
                if samples_are_aligned(&positions_info, &candidate, positions_info.values.len()) {
                    velocities_sample_time = candidate.sample_time;
                    velocities = candidate.values.clone();
                    velocities_info = Some(candidate);
                }
            }
        }

        let accelerations_attr = self.get_accelerations_attr();
        if accelerations_attr.is_valid() && !velocities.is_empty() {
            if let (Some(velocity_info), Some(candidate)) = (
                velocities_info.as_ref(),
                get_transform_sample::<usd_gf::vec3::Vec3f>(&accelerations_attr, base_time),
            ) {
                if samples_are_aligned(velocity_info, &candidate, positions_info.values.len()) {
                    accelerations = candidate.values;
                }
            }
        }

        // Positions: if no velocities, read at `time` (interpolated); else at lower bracket
        let positions_time = if velocities.is_empty() {
            time
        } else {
            positions_info.sample_time
        };
        let positions: Array<usd_gf::vec3::Vec3f> =
            if let Some(pos_value) = positions_attr.get(positions_time) {
                if let Some(arr) = extract_array::<usd_gf::vec3::Vec3f>(&pos_value) {
                    arr
                } else {
                    return false;
                }
            } else {
                return false;
            };

        // Scales: always at time for interpolation
        let mut scales: Array<usd_gf::vec3::Vec3f> = Array::new();
        let scales_attr = self.get_scales_attr();
        if scales_attr.is_valid() {
            if let Some(scale_value) = scales_attr.get(time) {
                if let Some(scale_arr) = extract_array::<usd_gf::vec3::Vec3f>(&scale_value) {
                    scales = scale_arr;
                }
            }
        }

        // Find lower bracket for orientations
        let (uses_orientationsf, orientations_attr) = self.uses_orientationsf();

        if uses_orientationsf {
            let orientations_info = if orientations_attr.is_valid() {
                get_transform_sample::<usd_gf::quat::Quatf>(&orientations_attr, base_time)
            } else {
                None
            };

            let mut angular_velocities: Array<usd_gf::vec3::Vec3f> = Array::new();
            let mut angular_velocities_sample_time = TimeCode::default_time();
            let angular_velocities_attr = self.get_angular_velocities_attr();
            if angular_velocities_attr.is_valid() {
                if let (Some(orientation_info), Some(candidate)) = (
                    orientations_info.as_ref(),
                    get_transform_sample::<usd_gf::vec3::Vec3f>(
                        &angular_velocities_attr,
                        base_time,
                    ),
                ) {
                    if samples_are_aligned(
                        orientation_info,
                        &candidate,
                        orientation_info.values.len(),
                    ) {
                        angular_velocities_sample_time = candidate.sample_time;
                        angular_velocities = candidate.values;
                    }
                }
            }

            let mut orientationsf: Array<usd_gf::quat::Quatf> = Array::new();
            if let Some(orientation_info) = orientations_info {
                if angular_velocities.is_empty() {
                    if let Some(orient_value) = orientations_attr.get(time) {
                        if let Some(orient_arr) =
                            extract_array::<usd_gf::quat::Quatf>(&orient_value)
                        {
                            orientationsf = orient_arr;
                        }
                    }
                } else {
                    orientationsf = orientation_info.values;
                }
            }
            self.do_compute_xforms_at_time(
                xforms,
                &stage,
                time,
                &proto_indices,
                &positions,
                &velocities,
                velocities_sample_time,
                &accelerations,
                &scales,
                &orientationsf,
                &angular_velocities,
                angular_velocities_sample_time,
                &proto_paths,
                &mask,
            )
        } else {
            let orientations_info = if orientations_attr.is_valid() {
                get_transform_sample::<usd_gf::quat::Quath>(&orientations_attr, base_time)
            } else {
                None
            };

            let mut angular_velocities: Array<usd_gf::vec3::Vec3f> = Array::new();
            let mut angular_velocities_sample_time = TimeCode::default_time();
            let angular_velocities_attr = self.get_angular_velocities_attr();
            if angular_velocities_attr.is_valid() {
                if let (Some(orientation_info), Some(candidate)) = (
                    orientations_info.as_ref(),
                    get_transform_sample::<usd_gf::vec3::Vec3f>(
                        &angular_velocities_attr,
                        base_time,
                    ),
                ) {
                    if samples_are_aligned(
                        orientation_info,
                        &candidate,
                        orientation_info.values.len(),
                    ) {
                        angular_velocities_sample_time = candidate.sample_time;
                        angular_velocities = candidate.values;
                    }
                }
            }

            let mut orientations: Array<usd_gf::quat::Quath> = Array::new();
            if let Some(orientation_info) = orientations_info {
                if angular_velocities.is_empty() {
                    if let Some(orient_value) = orientations_attr.get(time) {
                        if let Some(orient_arr) =
                            extract_array::<usd_gf::quat::Quath>(&orient_value)
                        {
                            orientations = orient_arr;
                        }
                    }
                } else {
                    orientations = orientation_info.values;
                }
            }
            self.do_compute_xforms_at_time(
                xforms,
                &stage,
                time,
                &proto_indices,
                &positions,
                &velocities,
                velocities_sample_time,
                &accelerations,
                &scales,
                &orientations,
                &angular_velocities,
                angular_velocities_sample_time,
                &proto_paths,
                &mask,
            )
        }
    }

    /// Compute the per-instance transforms at multiple times.
    ///
    /// Matches C++ `ComputeInstanceTransformsAtTimes()`.
    pub fn compute_instance_transforms_at_times(
        &self,
        xforms_array: &mut Vec<Vec<usd_gf::matrix4::Matrix4d>>,
        times: &[TimeCode],
        base_time: TimeCode,
        do_proto_xforms: ProtoXformInclusion,
        apply_mask: MaskApplication,
    ) -> bool {
        // Validate times
        for &t in times {
            if t.is_default() != base_time.is_default() {
                return false; // All times must be numeric or all default
            }
        }

        let (uses_orientationsf, orientations_attr) = self.uses_orientationsf();

        if uses_orientationsf {
            self.do_compute_xforms_at_times::<usd_gf::quat::Quatf>(
                xforms_array,
                times,
                base_time,
                do_proto_xforms,
                apply_mask,
                &orientations_attr,
            )
        } else {
            self.do_compute_xforms_at_times::<usd_gf::quat::Quath>(
                xforms_array,
                times,
                base_time,
                do_proto_xforms,
                apply_mask,
                &orientations_attr,
            )
        }
    }

    /// Generic helper: compute per-instance transforms across multiple times.
    ///
    /// Unifies C++ `_DoComputeInstanceTransformsAtTimes<GfQuatf>()` and
    /// `_DoComputeInstanceTransformsAtTimes<GfQuath>()` via `QuatToQuatd`.
    fn do_compute_xforms_at_times<Q: QuatToQuatd + Clone + Send + Sync + 'static>(
        &self,
        xforms_array: &mut Vec<Vec<usd_gf::matrix4::Matrix4d>>,
        times: &[TimeCode],
        base_time: TimeCode,
        do_proto_xforms: ProtoXformInclusion,
        apply_mask: MaskApplication,
        orientations_attr: &usd_core::Attribute,
    ) -> bool {
        let (proto_indices, proto_paths, mask) = match self
            .compute_point_instancer_attributes_preamble(base_time, do_proto_xforms, apply_mask)
        {
            Some(result) => result,
            None => return false,
        };

        let stage = match self.prim().stage() {
            Some(s) => s,
            None => return false,
        };

        let num_instances = proto_indices.len();
        let num_samples = times.len();

        if num_instances == 0 {
            xforms_array.clear();
            xforms_array.resize(num_samples, Vec::new());
            return true;
        }

        use usd_vt::Array;

        let positions_attr = self.get_positions_attr();
        if !positions_attr.is_valid() {
            return false;
        }

        let positions_info =
            match get_transform_sample::<usd_gf::vec3::Vec3f>(&positions_attr, base_time) {
                Some(info) => info,
                None => return false,
            };

        let mut velocities: Array<usd_gf::vec3::Vec3f> = Array::new();
        let mut accelerations: Array<usd_gf::vec3::Vec3f> = Array::new();
        let mut velocities_sample_time = TimeCode::default_time();
        let mut velocities_info = None;

        let velocities_attr = self.get_velocities_attr();
        if velocities_attr.is_valid() {
            if let Some(candidate) =
                get_transform_sample::<usd_gf::vec3::Vec3f>(&velocities_attr, base_time)
            {
                if samples_are_aligned(&positions_info, &candidate, positions_info.values.len()) {
                    velocities_sample_time = candidate.sample_time;
                    velocities = candidate.values.clone();
                    velocities_info = Some(candidate);
                }
            }
        }

        let accelerations_attr = self.get_accelerations_attr();
        if accelerations_attr.is_valid() && !velocities.is_empty() {
            if let (Some(velocity_info), Some(candidate)) = (
                velocities_info.as_ref(),
                get_transform_sample::<usd_gf::vec3::Vec3f>(&accelerations_attr, base_time),
            ) {
                if samples_are_aligned(velocity_info, &candidate, positions_info.values.len()) {
                    accelerations = candidate.values;
                }
            }
        }

        // Positions at lower bracket (used when velocities present)
        let positions = positions_info.values.clone();

        // Scales at base time
        let mut scales: Array<usd_gf::vec3::Vec3f> = Array::new();
        let scales_attr = self.get_scales_attr();
        if scales_attr.is_valid() {
            if let Some(scale_value) = scales_attr.get(base_time) {
                if let Some(scale_arr) = extract_array::<usd_gf::vec3::Vec3f>(&scale_value) {
                    scales = scale_arr;
                }
            }
        }

        let orientations_info = if orientations_attr.is_valid() {
            get_transform_sample::<Q>(orientations_attr, base_time)
        } else {
            None
        };

        let orientations: Array<Q> = orientations_info
            .as_ref()
            .map(|info| info.values.clone())
            .unwrap_or_default();
        let mut angular_velocities: Array<usd_gf::vec3::Vec3f> = Array::new();
        let mut angular_velocities_sample_time = TimeCode::default_time();

        let angular_velocities_attr = self.get_angular_velocities_attr();
        if angular_velocities_attr.is_valid() {
            if let (Some(orientation_info), Some(candidate)) = (
                orientations_info.as_ref(),
                get_transform_sample::<usd_gf::vec3::Vec3f>(&angular_velocities_attr, base_time),
            ) {
                if samples_are_aligned(orientation_info, &candidate, orientation_info.values.len())
                {
                    angular_velocities_sample_time = candidate.sample_time;
                    angular_velocities = candidate.values;
                }
            }
        }

        xforms_array.clear();
        xforms_array.reserve(num_samples);

        for &time in times {
            let mut xforms = Vec::new();
            xforms.reserve(num_instances);

            // Fetch per-sample values when interpolating (no velocity data)
            let mut sample_positions = positions.clone();
            let mut sample_scales = scales.clone();
            let mut sample_orientations = orientations.clone();

            if velocities.is_empty() {
                if let Some(pos_value) = positions_attr.get(time) {
                    if let Some(pos_arr) = extract_array::<usd_gf::vec3::Vec3f>(&pos_value) {
                        if pos_arr.len() == num_instances {
                            sample_positions = pos_arr;
                        }
                    }
                }
            }

            if scales_attr.is_valid() {
                if let Some(scale_value) = scales_attr.get(time) {
                    if let Some(scale_arr) = extract_array::<usd_gf::vec3::Vec3f>(&scale_value) {
                        if scale_arr.len() == num_instances {
                            sample_scales = scale_arr;
                        }
                    }
                }
            }

            if angular_velocities.is_empty() {
                if orientations_attr.is_valid() {
                    if let Some(orient_value) = orientations_attr.get(time) {
                        if let Some(orient_arr) = extract_array::<Q>(&orient_value) {
                            if orient_arr.len() == num_instances {
                                sample_orientations = orient_arr;
                            }
                        }
                    }
                }
            }

            if !self.do_compute_xforms_at_time(
                &mut xforms,
                &stage,
                time,
                &proto_indices,
                &sample_positions,
                &velocities,
                velocities_sample_time,
                &accelerations,
                &sample_scales,
                &sample_orientations,
                &angular_velocities,
                angular_velocities_sample_time,
                &proto_paths,
                &mask,
            ) {
                return false;
            }

            xforms_array.push(xforms);
        }

        true
    }

    /// Returns the number of instances at the given time.
    ///
    /// Matches C++ `GetInstanceCount()`.
    pub fn get_instance_count(&self, time_code: TimeCode) -> usize {
        let proto_indices_attr = self.get_proto_indices_attr();
        if let Some(value) = proto_indices_attr.get(time_code) {
            if let Some(arr) = value.get::<usd_vt::Array<i32>>() {
                return arr.len();
            }
            if let Some(vec) = value.get::<Vec<i32>>() {
                return vec.len();
            }
        }
        0
    }

    // ========================================================================
    // Extent Computation Methods
    // ========================================================================

    /// Helper to compute extent at time preamble.
    ///
    /// Matches C++ `_ComputeExtentAtTimePreamble()`.
    fn compute_extent_at_time_preamble(
        &self,
        base_time: TimeCode,
    ) -> Option<(
        usd_vt::Array<i32>,
        Vec<bool>,
        usd_core::Relationship,
        Vec<usd_sdf::Path>,
    )> {
        let proto_indices = self.get_proto_indices_for_instance_transforms(base_time)?;

        // Compute mask
        let mask = self.compute_mask_at_time(base_time, None);
        if !mask.is_empty() && mask.len() != proto_indices.len() {
            return None; // Mask size mismatch
        }

        // Get prototypes relationship
        let prototypes_rel = self.get_prototypes_rel();
        if !prototypes_rel.is_valid() {
            return None;
        }

        // Get prototype paths
        let proto_paths = prototypes_rel.get_targets();
        if proto_paths.is_empty() {
            return None;
        }

        // Validate all proto indices are in bounds
        for &proto_index in proto_indices.iter() {
            if proto_index < 0 || proto_index as usize >= proto_paths.len() {
                return None;
            }
        }

        Some((proto_indices, mask, prototypes_rel, proto_paths))
    }

    /// Helper to compute extent from transforms.
    ///
    /// Matches C++ `_ComputeExtentFromTransforms()`.
    fn compute_extent_from_transforms(
        &self,
        extent: &mut usd_vt::Array<usd_gf::vec3::Vec3f>,
        proto_indices: &usd_vt::Array<i32>,
        mask: &[bool],
        _prototypes_rel: &usd_core::Relationship,
        proto_paths: &[usd_sdf::Path],
        instance_transforms: &[usd_gf::matrix4::Matrix4d],
        time: TimeCode,
        transform: Option<&usd_gf::matrix4::Matrix4d>,
    ) -> bool {
        use super::bbox_cache::BBoxCache;
        use super::tokens::usd_geom_tokens;
        use usd_gf::{BBox3d, Range3d};

        let stage = match self.prim().stage() {
            Some(s) => s,
            None => return false,
        };

        // Compute prototype bounds
        let mut proto_untransformed_bounds = Vec::new();
        proto_untransformed_bounds.reserve(proto_paths.len());

        let purposes = vec![
            usd_geom_tokens().default_.clone(),
            usd_geom_tokens().proxy.clone(),
            usd_geom_tokens().render.clone(),
        ];
        let mut bbox_cache = BBoxCache::new(time, purposes, false, false);
        for proto_path in proto_paths {
            if let Some(proto_prim) = stage.get_prim_at_path(proto_path) {
                let proto_bbox = bbox_cache.compute_untransformed_bound(&proto_prim);
                proto_untransformed_bounds.push(proto_bbox);
            } else {
                proto_untransformed_bounds.push(BBox3d::new());
            }
        }

        // Compute all instance aligned ranges
        let mut instance_aligned_ranges = Vec::new();
        instance_aligned_ranges.reserve(proto_indices.len());

        for (instance_id, &proto_index) in proto_indices.iter().enumerate() {
            if !mask.is_empty() && !mask[instance_id] {
                continue;
            }

            if instance_id >= instance_transforms.len() {
                continue;
            }

            // Get the prototype bounding box; match C++ pattern: check >= 0 AND < len
            if proto_index < 0 || proto_index as usize >= proto_untransformed_bounds.len() {
                continue;
            }
            let proto_index_usize = proto_index as usize;

            let mut this_bounds = proto_untransformed_bounds[proto_index_usize];

            // Apply the instance transform
            this_bounds.transform(&instance_transforms[instance_id]);

            // Apply the optional transform
            if let Some(transform_mat) = transform {
                this_bounds.transform(transform_mat);
            }

            instance_aligned_ranges.push(this_bounds.compute_aligned_range());
        }

        // Union all ranges
        let mut extent_range = Range3d::empty();
        for range in &instance_aligned_ranges {
            extent_range.union_with(range);
        }

        let extent_min = *extent_range.min();
        let extent_max = *extent_range.max();

        *extent = usd_vt::Array::from(vec![
            usd_gf::vec3::Vec3f::new(
                extent_min.x as f32,
                extent_min.y as f32,
                extent_min.z as f32,
            ),
            usd_gf::vec3::Vec3f::new(
                extent_max.x as f32,
                extent_max.y as f32,
                extent_max.z as f32,
            ),
        ]);

        true
    }

    /// Compute extent at a single time.
    ///
    /// Matches C++ `ComputeExtentAtTime(extent, time, baseTime)`.
    pub fn compute_extent_at_time(
        &self,
        extent: &mut usd_vt::Array<usd_gf::vec3::Vec3f>,
        time: TimeCode,
        base_time: TimeCode,
    ) -> bool {
        self.compute_extent_at_time_with_transform(extent, time, base_time, None)
    }

    /// Compute extent at a single time with transform.
    ///
    /// Matches C++ `ComputeExtentAtTime(extent, time, baseTime, transform)`.
    pub fn compute_extent_at_time_with_transform(
        &self,
        extent: &mut usd_vt::Array<usd_gf::vec3::Vec3f>,
        time: TimeCode,
        base_time: TimeCode,
        transform: Option<&usd_gf::matrix4::Matrix4d>,
    ) -> bool {
        let (proto_indices, mask, prototypes_rel, proto_paths) =
            match self.compute_extent_at_time_preamble(base_time) {
                Some(result) => result,
                None => return false,
            };

        // Compute instance transforms (without masking)
        let mut instance_transforms = Vec::new();
        if !self.compute_instance_transforms_at_time(
            &mut instance_transforms,
            time,
            base_time,
            ProtoXformInclusion::IncludeProtoXform,
            MaskApplication::IgnoreMask,
        ) {
            return false;
        }

        self.compute_extent_from_transforms(
            extent,
            &proto_indices,
            &mask,
            &prototypes_rel,
            &proto_paths,
            &instance_transforms,
            time,
            transform,
        )
    }

    /// Compute extent at multiple times.
    ///
    /// Matches C++ `ComputeExtentAtTimes(extents, times, baseTime)`.
    pub fn compute_extent_at_times(
        &self,
        extents: &mut Vec<usd_vt::Array<usd_gf::vec3::Vec3f>>,
        times: &[TimeCode],
        base_time: TimeCode,
    ) -> bool {
        self.compute_extent_at_times_with_transform(extents, times, base_time, None)
    }

    /// Compute extent at multiple times with transform.
    ///
    /// Matches C++ `ComputeExtentAtTimes(extents, times, baseTime, transform)`.
    pub fn compute_extent_at_times_with_transform(
        &self,
        extents: &mut Vec<usd_vt::Array<usd_gf::vec3::Vec3f>>,
        times: &[TimeCode],
        base_time: TimeCode,
        transform: Option<&usd_gf::matrix4::Matrix4d>,
    ) -> bool {
        let (proto_indices, mask, prototypes_rel, proto_paths) =
            match self.compute_extent_at_time_preamble(base_time) {
                Some(result) => result,
                None => return false,
            };

        // Compute instance transforms for all times (without masking)
        let mut instance_transforms_array = Vec::new();
        if !self.compute_instance_transforms_at_times(
            &mut instance_transforms_array,
            times,
            base_time,
            ProtoXformInclusion::IncludeProtoXform,
            MaskApplication::IgnoreMask,
        ) {
            return false;
        }

        extents.clear();
        extents.reserve(times.len());

        for (i, &time) in times.iter().enumerate() {
            if i >= instance_transforms_array.len() {
                return false;
            }

            let mut extent = usd_vt::Array::new();
            if !self.compute_extent_from_transforms(
                &mut extent,
                &proto_indices,
                &mask,
                &prototypes_rel,
                &proto_paths,
                &instance_transforms_array[i],
                time,
                transform,
            ) {
                return false;
            }
            extents.push(extent);
        }

        true
    }

    // ========================================================================
    // Static ComputeInstanceTransformsAtTime Methods
    // ========================================================================

    /// Static method to compute instance transforms with Quatf orientations.
    ///
    /// Matches C++ `ComputeInstanceTransformsAtTime(..., VtQuatfArray, ...)`.
    pub fn compute_instance_transforms_at_time_static_quatf(
        xforms: &mut Vec<usd_gf::matrix4::Matrix4d>,
        stage: &usd_core::Stage,
        time: TimeCode,
        proto_indices: &usd_vt::Array<i32>,
        positions: &usd_vt::Array<usd_gf::vec3::Vec3f>,
        velocities: &usd_vt::Array<usd_gf::vec3::Vec3f>,
        velocities_sample_time: TimeCode,
        accelerations: &usd_vt::Array<usd_gf::vec3::Vec3f>,
        scales: &usd_vt::Array<usd_gf::vec3::Vec3f>,
        orientations: &usd_vt::Array<usd_gf::quat::Quatf>,
        angular_velocities: &usd_vt::Array<usd_gf::vec3::Vec3f>,
        angular_velocities_sample_time: TimeCode,
        proto_paths: &[usd_sdf::Path],
        mask: &[bool],
    ) -> bool {
        Self::compute_xforms_at_time_impl(
            xforms,
            stage,
            time,
            proto_indices,
            positions,
            velocities,
            velocities_sample_time,
            accelerations,
            scales,
            orientations,
            angular_velocities,
            angular_velocities_sample_time,
            proto_paths,
            mask,
            Self::calculate_time_delta_static,
        )
    }

    /// Static method to compute instance transforms with Quath orientations.
    ///
    /// Matches C++ `ComputeInstanceTransformsAtTime(..., VtQuathArray, ...)`.
    pub fn compute_instance_transforms_at_time_static_quath(
        xforms: &mut Vec<usd_gf::matrix4::Matrix4d>,
        stage: &usd_core::Stage,
        time: TimeCode,
        proto_indices: &usd_vt::Array<i32>,
        positions: &usd_vt::Array<usd_gf::vec3::Vec3f>,
        velocities: &usd_vt::Array<usd_gf::vec3::Vec3f>,
        velocities_sample_time: TimeCode,
        accelerations: &usd_vt::Array<usd_gf::vec3::Vec3f>,
        scales: &usd_vt::Array<usd_gf::vec3::Vec3f>,
        orientations: &usd_vt::Array<usd_gf::quat::Quath>,
        angular_velocities: &usd_vt::Array<usd_gf::vec3::Vec3f>,
        angular_velocities_sample_time: TimeCode,
        proto_paths: &[usd_sdf::Path],
        mask: &[bool],
    ) -> bool {
        Self::compute_xforms_at_time_impl(
            xforms,
            stage,
            time,
            proto_indices,
            positions,
            velocities,
            velocities_sample_time,
            accelerations,
            scales,
            orientations,
            angular_velocities,
            angular_velocities_sample_time,
            proto_paths,
            mask,
            Self::calculate_time_delta_static,
        )
    }

    /// Static helper to calculate time delta.
    fn calculate_time_delta_static(
        time: TimeCode,
        sample_time: TimeCode,
        time_codes_per_second: f64,
    ) -> f64 {
        if time.is_default() || sample_time.is_default() {
            return 0.0;
        }
        (time.value() - sample_time.value()) / time_codes_per_second
    }
}

impl PartialEq for PointInstancer {
    fn eq(&self, other: &Self) -> bool {
        self.inner == other.inner
    }
}

impl Eq for PointInstancer {}
