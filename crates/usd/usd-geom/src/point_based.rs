//! UsdGeomPointBased - base class for all point-based geometric primitives.
//!
//! Port of pxr/usd/usdGeom/pointBased.h/cpp
//!
//! Base class for all UsdGeomGprims that possess points,
//! providing common attributes such as normals and velocities.

use super::gprim::Gprim;
use super::tokens::usd_geom_tokens;
use usd_core::attribute::Variability;
use usd_core::{Attribute, Prim, Stage};
use usd_gf::matrix4::Matrix4d;
use usd_gf::vec3::Vec3f;
use usd_sdf::TimeCode;
use usd_sdf::ValueTypeRegistry;
use usd_tf::Token;
use usd_vt::Value;

// ============================================================================
// Transform sampling helpers
// ============================================================================

#[derive(Clone)]
struct TransformSampleInfo<T> {
    sample_time: TimeCode,
    lower_time_value: f64,
    upper_time_value: f64,
    has_samples: bool,
    values: Vec<T>,
}

fn extract_vec3f_array(value: &Value) -> Option<Vec<Vec3f>> {
    value.get::<Vec<Vec3f>>().cloned().or_else(|| {
        value
            .get::<usd_vt::Array<Vec3f>>()
            .map(|arr| arr.iter().cloned().collect())
    })
}

fn times_close(lhs: f64, rhs: f64) -> bool {
    (lhs - rhs).abs() <= f64::EPSILON
}

fn get_vec3f_transform_sample(
    attr: &Attribute,
    base_time: TimeCode,
) -> Option<TransformSampleInfo<Vec3f>> {
    if base_time.is_default() {
        let value = attr.get(base_time)?;
        return Some(TransformSampleInfo {
            sample_time: base_time,
            lower_time_value: base_time.value(),
            upper_time_value: base_time.value(),
            has_samples: false,
            values: extract_vec3f_array(&value)?,
        });
    }

    let value_time = if let Some((lower, upper)) =
        attr.get_bracketing_time_samples(base_time.value())
    {
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
            values: extract_vec3f_array(&value)?,
        });
    } else {
        TimeCode::default_time()
    };

    let value = attr.get(value_time)?;
    Some(TransformSampleInfo {
        sample_time: value_time,
        lower_time_value: base_time.value(),
        upper_time_value: base_time.value(),
        has_samples: false,
        values: extract_vec3f_array(&value)?,
    })
}

fn samples_are_aligned<T>(
    reference: &TransformSampleInfo<T>,
    candidate: &TransformSampleInfo<T>,
    expected_len: usize,
) -> bool {
    reference.has_samples
        && times_close(reference.lower_time_value, candidate.lower_time_value)
        && times_close(reference.upper_time_value, candidate.upper_time_value)
        && times_close(reference.sample_time.value(), candidate.sample_time.value())
        && candidate.values.len() == expected_len
}

// ============================================================================
// PointBased
// ============================================================================

/// Base class for all point-based geometric primitives.
///
/// PointBased provides common attributes such as points, normals, velocities,
/// and accelerations that are shared by all point-based primitives like Mesh,
/// Points, and Curves.
///
/// Matches C++ `UsdGeomPointBased`.
#[derive(Debug, Clone)]
pub struct PointBased {
    /// Base gprim schema.
    inner: Gprim,
}

impl PointBased {
    /// Creates a PointBased schema from a prim.
    ///
    /// Matches C++ `UsdGeomPointBased(const UsdPrim& prim)`.
    pub fn new(prim: Prim) -> Self {
        Self {
            inner: Gprim::new(prim),
        }
    }

    /// Creates a PointBased schema from a Gprim schema.
    ///
    /// Matches C++ `UsdGeomPointBased(const UsdSchemaBase& schemaObj)`.
    pub fn from_gprim(gprim: Gprim) -> Self {
        Self { inner: gprim }
    }

    /// Creates an invalid PointBased schema.
    pub fn invalid() -> Self {
        Self {
            inner: Gprim::invalid(),
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

    /// Returns the gprim base.
    pub fn gprim(&self) -> &Gprim {
        &self.inner
    }

    /// Returns the schema type name.
    pub fn schema_type_name() -> Token {
        Token::new("PointBased")
    }

    /// Return a PointBased holding the prim adhering to this schema at path on stage.
    ///
    /// Matches C++ `UsdGeomPointBased::Get(const UsdStagePtr &stage, const SdfPath &path)`.
    pub fn get(stage: &Stage, path: &usd_sdf::Path) -> Self {
        if let Some(prim) = stage.get_prim_at_path(path) {
            Self::new(prim)
        } else {
            Self::invalid()
        }
    }

    // ========================================================================
    // Points
    // ========================================================================

    /// Returns the points attribute.
    ///
    /// The primary geometry attribute for all PointBased primitives,
    /// describes points in (local) space.
    ///
    /// Matches C++ `GetPointsAttr()`.
    pub fn get_points_attr(&self) -> Attribute {
        let prim = self.inner.prim();
        prim.get_attribute(usd_geom_tokens().points.as_str())
            .unwrap_or_else(Attribute::invalid)
    }

    /// Creates the points attribute.
    ///
    /// Matches C++ `CreatePointsAttr()`.
    pub fn create_points_attr(
        &self,
        default_value: Option<Value>,
        _write_sparsely: bool,
    ) -> Attribute {
        let prim = self.inner.prim();
        if !prim.is_valid() {
            return Attribute::invalid();
        }

        let registry = ValueTypeRegistry::instance();
        let point3f_array_type = registry.find_type_by_token(&Token::new("point3f[]"));

        let attr = prim
            .create_attribute(
                usd_geom_tokens().points.as_str(),
                &point3f_array_type,
                false,                      // not custom
                Some(Variability::Varying), // can vary over time
            )
            .unwrap_or_else(Attribute::invalid);
        if let Some(val) = default_value {
            let _ = attr.set(val, TimeCode::default());
        }
        attr
    }

    // ========================================================================
    // Velocities
    // ========================================================================

    /// Returns the velocities attribute.
    ///
    /// If provided, 'velocities' should be used by renderers to compute
    /// positions between samples for the 'points' attribute, rather than
    /// interpolating between neighboring 'points' samples.
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
        _default_value: Option<Value>,
        _write_sparsely: bool,
    ) -> Attribute {
        let prim = self.inner.prim();
        if !prim.is_valid() {
            return Attribute::invalid();
        }

        if prim.has_authored_attribute(usd_geom_tokens().velocities.as_str()) {
            return prim
                .get_attribute(usd_geom_tokens().velocities.as_str())
                .unwrap_or_else(|| Attribute::invalid());
        }

        let registry = ValueTypeRegistry::instance();
        let vector3f_array_type = registry.find_type_by_token(&Token::new("vector3f[]"));

        prim.create_attribute(
            usd_geom_tokens().velocities.as_str(),
            &vector3f_array_type,
            false,                      // not custom
            Some(Variability::Varying), // can vary over time
        )
        .unwrap_or_else(Attribute::invalid)
    }

    // ========================================================================
    // Accelerations
    // ========================================================================

    /// Returns the accelerations attribute.
    ///
    /// If provided, 'accelerations' should be used with velocities to compute
    /// positions between samples for the 'points' attribute.
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
        _default_value: Option<Value>,
        _write_sparsely: bool,
    ) -> Attribute {
        let prim = self.inner.prim();
        if !prim.is_valid() {
            return Attribute::invalid();
        }

        if prim.has_authored_attribute(usd_geom_tokens().accelerations.as_str()) {
            return prim
                .get_attribute(usd_geom_tokens().accelerations.as_str())
                .unwrap_or_else(|| Attribute::invalid());
        }

        let registry = ValueTypeRegistry::instance();
        let vector3f_array_type = registry.find_type_by_token(&Token::new("vector3f[]"));

        prim.create_attribute(
            usd_geom_tokens().accelerations.as_str(),
            &vector3f_array_type,
            false,                      // not custom
            Some(Variability::Varying), // can vary over time
        )
        .unwrap_or_else(Attribute::invalid)
    }

    // ========================================================================
    // Normals
    // ========================================================================

    /// Returns the normals attribute.
    ///
    /// Provide an object-space orientation for individual points.
    ///
    /// Matches C++ `GetNormalsAttr()`.
    pub fn get_normals_attr(&self) -> Attribute {
        let prim = self.inner.prim();
        prim.get_attribute(usd_geom_tokens().normals.as_str())
            .unwrap_or_else(Attribute::invalid)
    }

    /// Creates the normals attribute.
    ///
    /// Matches C++ `CreateNormalsAttr()`.
    pub fn create_normals_attr(
        &self,
        _default_value: Option<Value>,
        _write_sparsely: bool,
    ) -> Attribute {
        let prim = self.inner.prim();
        if !prim.is_valid() {
            return Attribute::invalid();
        }

        if prim.has_authored_attribute(usd_geom_tokens().normals.as_str()) {
            return prim
                .get_attribute(usd_geom_tokens().normals.as_str())
                .unwrap_or_else(|| Attribute::invalid());
        }

        let registry = ValueTypeRegistry::instance();
        let normal3f_array_type = registry.find_type_by_token(&Token::new("normal3f[]"));

        prim.create_attribute(
            usd_geom_tokens().normals.as_str(),
            &normal3f_array_type,
            false,                      // not custom
            Some(Variability::Varying), // can vary over time
        )
        .unwrap_or_else(Attribute::invalid)
    }

    /// Get the interpolation for the normals attribute.
    ///
    /// The fallback interpolation, if left unspecified, is "vertex".
    ///
    /// Matches C++ `GetNormalsInterpolation()`.
    pub fn get_normals_interpolation(&self) -> Token {
        let normals_attr = self.get_normals_attr();
        if !normals_attr.is_valid() {
            return usd_geom_tokens().vertex.clone();
        }

        // Check for interpolation metadata
        if let Some(interp_value) = normals_attr.get_metadata(&usd_geom_tokens().interpolation) {
            // Try to extract token from value
            if let Some(interp_str) = interp_value.get::<String>() {
                return Token::new(interp_str);
            } else if let Some(interp_str) = interp_value.get::<Token>() {
                return interp_str.clone();
            }
        }

        usd_geom_tokens().vertex.clone()
    }

    /// Set the interpolation for the normals attribute.
    ///
    /// Returns true upon success, false if interpolation is not a legal value.
    ///
    /// Matches C++ `SetNormalsInterpolation()`.
    pub fn set_normals_interpolation(&self, interpolation: &Token) -> bool {
        // Validate interpolation - valid values are: constant, uniform, varying, vertex, faceVarying
        let interp_str = interpolation.as_str();
        if interp_str != usd_geom_tokens().constant.as_str()
            && interp_str != usd_geom_tokens().uniform.as_str()
            && interp_str != usd_geom_tokens().varying.as_str()
            && interp_str != usd_geom_tokens().vertex.as_str()
            && interp_str != usd_geom_tokens().face_varying.as_str()
        {
            return false;
        }

        let normals_attr = self.get_normals_attr();
        if !normals_attr.is_valid() {
            return false;
        }

        normals_attr.set_metadata(
            &usd_geom_tokens().interpolation,
            interpolation.as_str().to_string(),
        )
    }

    // ========================================================================
    // Compute Extent
    // ========================================================================

    /// Compute the extent for the point cloud defined by points.
    ///
    /// Returns true on success, false if extents was unable to be calculated.
    /// On success, extent will contain the axis-aligned bounding box of the
    /// point cloud defined by points.
    ///
    /// Matches C++ `ComputeExtent(const VtVec3fArray& points, VtVec3fArray* extent)`.
    pub fn compute_extent(points: &[Vec3f], extent: &mut [Vec3f; 2]) -> bool {
        if points.is_empty() {
            return false;
        }

        let mut min = Vec3f::new(f32::MAX, f32::MAX, f32::MAX);
        let mut max = Vec3f::new(f32::MIN, f32::MIN, f32::MIN);

        for point in points {
            min.x = min.x.min(point.x);
            min.y = min.y.min(point.y);
            min.z = min.z.min(point.z);
            max.x = max.x.max(point.x);
            max.y = max.y.max(point.y);
            max.z = max.z.max(point.z);
        }

        extent[0] = min;
        extent[1] = max;
        true
    }

    /// Compute the extent as if the matrix transform was first applied.
    ///
    /// Matches C++ `ComputeExtent(const VtVec3fArray& points, const GfMatrix4d& transform, VtVec3fArray* extent)`.
    pub fn compute_extent_with_transform(
        points: &[Vec3f],
        transform: &Matrix4d,
        extent: &mut [Vec3f; 2],
    ) -> bool {
        if points.is_empty() {
            return false;
        }

        let mut min = Vec3f::new(f32::MAX, f32::MAX, f32::MAX);
        let mut max = Vec3f::new(f32::MIN, f32::MIN, f32::MIN);

        for point in points {
            // Convert Vec3f to Vec3<f64> for transform, then back to Vec3f
            let point_d = usd_gf::vec3::Vec3d::new(point.x as f64, point.y as f64, point.z as f64);
            let transformed_d = transform.transform_point(&point_d);
            let transformed = Vec3f::new(
                transformed_d.x as f32,
                transformed_d.y as f32,
                transformed_d.z as f32,
            );
            min.x = min.x.min(transformed.x);
            min.y = min.y.min(transformed.y);
            min.z = min.z.min(transformed.z);
            max.x = max.x.max(transformed.x);
            max.y = max.y.max(transformed.y);
            max.z = max.z.max(transformed.z);
        }

        extent[0] = min;
        extent[1] = max;
        true
    }

    // ========================================================================
    // Compute Points At Time
    // ========================================================================

    /// Compute points given the positions, velocities and accelerations at time.
    ///
    /// This will return false and leave points untouched if:
    /// - points is None
    /// - one of time and base_time is numeric and the other is Default (they must either both be numeric or both be default)
    /// - there is no authored points attribute
    ///
    /// Matches C++ `ComputePointsAtTime(VtArray<GfVec3f>* points, const UsdTimeCode time, const UsdTimeCode baseTime)`.
    pub fn compute_points_at_time(
        &self,
        points: &mut Vec<Vec3f>,
        time: TimeCode,
        base_time: TimeCode,
    ) -> bool {
        // Validate that both times are either numeric or both default
        if time.is_default() != base_time.is_default() {
            return false;
        }

        let points_attr = self.get_points_attr();
        if !points_attr.is_valid() {
            return false;
        }

        let positions_info = match get_vec3f_transform_sample(&points_attr, base_time) {
            Some(info) => info,
            None => return false,
        };

        let mut velocities = Vec::new();
        let mut velocities_info = None;
        let velocities_attr = self.get_velocities_attr();
        let mut velocities_sample_time = TimeCode::default_time();
        if velocities_attr.is_valid() {
            if let Some(candidate) = get_vec3f_transform_sample(&velocities_attr, base_time) {
                if samples_are_aligned(&positions_info, &candidate, positions_info.values.len()) {
                    velocities_sample_time = candidate.sample_time;
                    velocities = candidate.values.clone();
                    velocities_info = Some(candidate);
                }
            }
        }

        let mut accelerations = Vec::new();
        let accelerations_attr = self.get_accelerations_attr();
        if accelerations_attr.is_valid() && !velocities.is_empty() {
            if let (Some(velocity_info), Some(candidate)) = (
                velocities_info.as_ref(),
                get_vec3f_transform_sample(&accelerations_attr, base_time),
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
        let positions_value = match points_attr.get(positions_time) {
            Some(v) => v,
            None => return false,
        };
        let positions: Vec<Vec3f> = match extract_vec3f_array(&positions_value) {
            Some(arr) => arr,
            None => return false,
        };

        // Get stage for time codes per second
        let stage = match self.prim().stage() {
            Some(s) => s,
            None => return false,
        };

        Self::compute_points_at_time_static(
            points,
            stage.as_ref(),
            time,
            &positions,
            &velocities,
            velocities_sample_time,
            &accelerations,
        )
    }

    /// Static version of ComputePointsAtTime that takes all data as parameters.
    ///
    /// Matches C++ `ComputePointsAtTime(VtArray<GfVec3f>* points, UsdStageWeakPtr& stage, UsdTimeCode time, const VtVec3fArray& positions, const VtVec3fArray& velocities, UsdTimeCode velocitiesSampleTime, const VtVec3fArray& accelerations, float velocityScale)`.
    pub fn compute_points_at_time_static(
        points: &mut Vec<Vec3f>,
        stage: &Stage,
        time: TimeCode,
        positions: &[Vec3f],
        velocities: &[Vec3f],
        velocities_sample_time: TimeCode,
        accelerations: &[Vec3f],
    ) -> bool {
        let num_points = positions.len();

        // Validate velocities and accelerations sizes
        if !velocities.is_empty() && velocities.len() != num_points {
            return false;
        }
        if !accelerations.is_empty() && accelerations.len() != num_points {
            return false;
        }

        let time_codes_per_second = stage.get_time_codes_per_second();

        // Calculate time delta
        let velocity_time_delta = if !time.is_default() && !velocities_sample_time.is_default() {
            let time_val = time.value();
            let base_val = velocities_sample_time.value();
            (time_val - base_val) / time_codes_per_second
        } else {
            0.0
        };

        points.clear();
        points.reserve(num_points);

        for i in 0..num_points {
            let mut translation = positions[i];

            if !velocities.is_empty() {
                let mut velocity = velocities[i];
                if !accelerations.is_empty() {
                    let accel = accelerations[i];
                    let accel_scaled = Vec3f::new(
                        (velocity_time_delta * accel.x as f64 * 0.5) as f32,
                        (velocity_time_delta * accel.y as f64 * 0.5) as f32,
                        (velocity_time_delta * accel.z as f64 * 0.5) as f32,
                    );
                    velocity += accel_scaled;
                }
                let vel_scaled = Vec3f::new(
                    (velocity_time_delta * velocity.x as f64) as f32,
                    (velocity_time_delta * velocity.y as f64) as f32,
                    (velocity_time_delta * velocity.z as f64) as f32,
                );
                translation += vel_scaled;
            }

            points.push(translation);
        }

        true
    }

    /// Compute points as in ComputePointsAtTime, but using multiple sample times.
    ///
    /// Matches C++ `ComputePointsAtTimes(std::vector<VtArray<GfVec3f>>* pointsArray, const std::vector<UsdTimeCode>& times, const UsdTimeCode baseTime)`.
    pub fn compute_points_at_times(
        &self,
        points_array: &mut Vec<Vec<Vec3f>>,
        times: &[TimeCode],
        base_time: TimeCode,
    ) -> bool {
        let num_samples = times.len();

        // Validate that all times are either numeric or all default
        for time in times {
            if time.is_default() != base_time.is_default() {
                return false;
            }
        }

        let points_attr = self.get_points_attr();
        if !points_attr.is_valid() {
            return false;
        }

        let positions_info = match get_vec3f_transform_sample(&points_attr, base_time) {
            Some(info) => info,
            None => return false,
        };

        let mut velocities = Vec::new();
        let mut velocities_info = None;
        let velocities_attr = self.get_velocities_attr();
        let mut velocities_sample_time = TimeCode::default_time();
        if velocities_attr.is_valid() {
            if let Some(candidate) = get_vec3f_transform_sample(&velocities_attr, base_time) {
                if samples_are_aligned(&positions_info, &candidate, positions_info.values.len()) {
                    velocities_sample_time = candidate.sample_time;
                    velocities = candidate.values.clone();
                    velocities_info = Some(candidate);
                }
            }
        }

        let mut accelerations = Vec::new();
        let accelerations_attr = self.get_accelerations_attr();
        if accelerations_attr.is_valid() && !velocities.is_empty() {
            if let (Some(velocity_info), Some(candidate)) = (
                velocities_info.as_ref(),
                get_vec3f_transform_sample(&accelerations_attr, base_time),
            ) {
                if samples_are_aligned(velocity_info, &candidate, positions_info.values.len()) {
                    accelerations = candidate.values;
                }
            }
        }

        let positions = positions_info.values.clone();

        let num_points = positions.len();
        if num_points == 0 {
            points_array.clear();
            points_array.resize(num_samples, Vec::new());
            return true;
        }

        let stage = match self.prim().stage() {
            Some(s) => s,
            None => return false,
        };

        let use_interpolated = velocities.is_empty();

        points_array.clear();
        points_array.reserve(num_samples);

        for time in times {
            let mut computed_points = Vec::new();

            // If there are no valid velocities, try to fetch interpolated points
            if use_interpolated {
                if let Some(interp_value) = points_attr.get(*time) {
                    let interp_points: Option<Vec<Vec3f>> =
                        interp_value.get::<Vec<Vec3f>>().cloned().or_else(|| {
                            interp_value
                                .get::<usd_vt::Array<Vec3f>>()
                                .map(|arr| arr.iter().cloned().collect())
                        });

                    if let Some(interp_points) = interp_points {
                        if interp_points.len() == num_points {
                            // Use interpolated points as positions
                            Self::compute_points_at_time_static(
                                &mut computed_points,
                                stage.as_ref(),
                                *time,
                                &interp_points,
                                &velocities,
                                velocities_sample_time,
                                &accelerations,
                            );
                            points_array.push(computed_points);
                            continue;
                        }
                    }
                }
            }

            // Fallback to base time positions or use velocity-based computation
            Self::compute_points_at_time_static(
                &mut computed_points,
                stage.as_ref(),
                *time,
                &positions,
                &velocities,
                velocities_sample_time,
                &accelerations,
            );

            points_array.push(computed_points);
        }

        true
    }

    /// Return a vector of names of all pre-declared attributes for this schema
    /// class and all its ancestor classes.
    ///
    /// Matches C++ `GetSchemaAttributeNames(bool includeInherited)`.
    pub fn get_schema_attribute_names(include_inherited: bool) -> Vec<Token> {
        let local_names = vec![
            usd_geom_tokens().points.clone(),
            usd_geom_tokens().velocities.clone(),
            usd_geom_tokens().accelerations.clone(),
            usd_geom_tokens().normals.clone(),
        ];

        if include_inherited {
            let mut all_names = Gprim::get_schema_attribute_names(true);
            all_names.extend(local_names);
            all_names
        } else {
            local_names
        }
    }
}

impl PartialEq for PointBased {
    fn eq(&self, other: &Self) -> bool {
        self.inner == other.inner
    }
}

impl Eq for PointBased {}
