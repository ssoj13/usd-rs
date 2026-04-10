//! UsdGeomPoints - points geometry schema.
//!
//! Port of pxr/usd/usdGeom/points.h/cpp
//!
//! Points are analogous to the RiPoints spec. Points can be an efficient means
//! of storing and rendering particle effects comprised of thousands or millions
//! of small particles.

use super::point_based::PointBased;
use super::schema_create_default::apply_optional_default;
use super::tokens::usd_geom_tokens;
use usd_core::attribute::Variability;
use usd_core::{Attribute, Prim, Stage};
use usd_gf::matrix4::Matrix4d;
use usd_gf::vec3::Vec3f;
use usd_sdf::{TimeCode, ValueTypeRegistry};
use usd_tf::Token;
use usd_vt::Value;

/// Extract `float[]` / `double[]` / scalar widths from a composed `VtValue`.
pub(crate) fn widths_vec_from_usd_value(widths_value: &Value) -> Vec<f32> {
    if let Some(w) = widths_value.get::<Vec<f32>>() {
        return w.clone();
    }
    if let Some(w) = widths_value.get::<usd_vt::Array<f32>>() {
        return w.iter().cloned().collect();
    }
    if let Some(w) = widths_value.get::<Vec<f64>>() {
        return w.iter().map(|&x| x as f32).collect();
    }
    if let Some(w) = widths_value.get::<usd_vt::Array<f64>>() {
        return w.iter().map(|&x| x as f32).collect();
    }
    if let Some(s) = widths_value.get::<f32>() {
        return vec![*s];
    }
    if let Some(s) = widths_value.get::<f64>() {
        return vec![*s as f32];
    }
    if let Some(w) = widths_value.get::<Vec<i32>>() {
        return w.iter().map(|&x| x as f32).collect();
    }
    if let Some(w) = widths_value.get::<usd_vt::Array<i32>>() {
        return w.iter().map(|&x| x as f32).collect();
    }
    if let Some(w) = widths_value.get::<Vec<i64>>() {
        return w.iter().map(|&x| x as f32).collect();
    }
    if let Some(w) = widths_value.get::<usd_vt::Array<i64>>() {
        return w.iter().map(|&x| x as f32).collect();
    }
    Vec::new()
}

// ============================================================================
// Points
// ============================================================================

/// Points geometry schema.
///
/// Points are analogous to the RiPoints spec. Points can be an efficient means
/// of storing and rendering particle effects comprised of thousands or millions
/// of small particles.
///
/// Matches C++ `UsdGeomPoints`.
#[derive(Debug, Clone)]
pub struct Points {
    /// Base point-based schema.
    inner: PointBased,
}

impl Points {
    /// Creates a Points schema from a prim.
    ///
    /// Matches C++ `UsdGeomPoints(const UsdPrim& prim)`.
    pub fn new(prim: Prim) -> Self {
        Self {
            inner: PointBased::new(prim),
        }
    }

    /// Creates a Points schema from a PointBased schema.
    ///
    /// Matches C++ `UsdGeomPoints(const UsdSchemaBase& schemaObj)`.
    pub fn from_point_based(point_based: PointBased) -> Self {
        Self { inner: point_based }
    }

    /// Creates an invalid Points schema.
    pub fn invalid() -> Self {
        Self {
            inner: PointBased::invalid(),
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

    /// Returns the point-based base.
    pub fn point_based(&self) -> &PointBased {
        &self.inner
    }

    /// Returns the schema type name.
    pub fn schema_type_name() -> Token {
        Token::new("Points")
    }

    /// Return a Points holding the prim adhering to this schema at path on stage.
    ///
    /// Matches C++ `UsdGeomPoints::Get(const UsdStagePtr &stage, const SdfPath &path)`.
    pub fn get(stage: &Stage, path: &usd_sdf::Path) -> Self {
        if let Some(prim) = stage.get_prim_at_path(path) {
            Self::new(prim)
        } else {
            Self::invalid()
        }
    }

    /// Attempt to ensure a prim adhering to this schema at path is defined on this stage.
    ///
    /// Matches C++ `UsdGeomPoints::Define(const UsdStagePtr &stage, const SdfPath &path)`.
    pub fn define(stage: &Stage, path: &usd_sdf::Path) -> Self {
        match stage.define_prim(path.get_string(), Self::schema_type_name().as_str()) {
            Ok(prim) => Self::new(prim),
            Err(_) => Self::invalid(),
        }
    }

    // ========================================================================
    // Widths
    // ========================================================================

    /// Returns the widths attribute.
    ///
    /// Widths are defined as the diameter of the points, in object space.
    ///
    /// Matches C++ `GetWidthsAttr()`.
    pub fn get_widths_attr(&self) -> Attribute {
        let prim = self.inner.prim();
        prim.get_attribute(usd_geom_tokens().widths.as_str())
            .unwrap_or_else(Attribute::invalid)
    }

    /// Creates the widths attribute.
    ///
    /// Matches C++ `CreateWidthsAttr(VtValue defaultValue, bool writeSparsely)`.
    pub fn create_widths_attr(
        &self,
        default_value: Option<Value>,
        _write_sparsely: bool,
    ) -> Attribute {
        let prim = self.inner.prim();
        if !prim.is_valid() {
            return Attribute::invalid();
        }

        let attr = if prim.has_authored_attribute(usd_geom_tokens().widths.as_str()) {
            prim.get_attribute(usd_geom_tokens().widths.as_str())
                .unwrap_or_else(Attribute::invalid)
        } else {
            let registry = ValueTypeRegistry::instance();
            let float_array_type = registry.find_type_by_token(&Token::new("float[]"));

            prim.create_attribute(
                usd_geom_tokens().widths.as_str(),
                &float_array_type,
                false,
                Some(Variability::Varying),
            )
            .unwrap_or_else(Attribute::invalid)
        };

        apply_optional_default(attr, default_value)
    }

    /// Get the interpolation for the widths attribute.
    ///
    /// The fallback interpolation, if left unspecified, is "vertex".
    ///
    /// Matches C++ `GetWidthsInterpolation()`.
    pub fn get_widths_interpolation(&self) -> Token {
        let widths_attr = self.get_widths_attr();
        if !widths_attr.is_valid() {
            return usd_geom_tokens().vertex.clone();
        }

        // Check for interpolation metadata
        if let Some(interp_value) = widths_attr.get_metadata(&usd_geom_tokens().interpolation) {
            // Try to extract token from value
            if let Some(interp_str) = interp_value.get::<String>() {
                return Token::new(interp_str);
            } else if let Some(interp_token) = interp_value.get::<Token>() {
                return interp_token.clone();
            }
        }

        usd_geom_tokens().vertex.clone()
    }

    /// Set the interpolation for the widths attribute.
    ///
    /// Returns true upon success, false if interpolation is not a legal value.
    ///
    /// Matches C++ `SetWidthsInterpolation()`.
    pub fn set_widths_interpolation(&self, interpolation: &Token) -> bool {
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

        let widths_attr = self.get_widths_attr();
        if !widths_attr.is_valid() {
            return false;
        }

        widths_attr.set_metadata(
            &usd_geom_tokens().interpolation,
            interpolation.as_str().to_string(),
        )
    }

    // ========================================================================
    // Ids
    // ========================================================================

    /// Returns the ids attribute.
    ///
    /// Ids are optional; if authored, the ids array should be the same
    /// length as the points array, specifying the id of each point.
    ///
    /// Matches C++ `GetIdsAttr()`.
    pub fn get_ids_attr(&self) -> Attribute {
        let prim = self.inner.prim();
        prim.get_attribute(usd_geom_tokens().ids.as_str())
            .unwrap_or_else(Attribute::invalid)
    }

    /// Creates the ids attribute.
    ///
    /// Matches C++ `CreateIdsAttr(VtValue defaultValue, bool writeSparsely)`.
    pub fn create_ids_attr(
        &self,
        default_value: Option<Value>,
        _write_sparsely: bool,
    ) -> Attribute {
        let prim = self.inner.prim();
        if !prim.is_valid() {
            return Attribute::invalid();
        }

        let attr = if prim.has_authored_attribute(usd_geom_tokens().ids.as_str()) {
            prim.get_attribute(usd_geom_tokens().ids.as_str())
                .unwrap_or_else(Attribute::invalid)
        } else {
            let registry = ValueTypeRegistry::instance();
            let int64_array_type = registry.find_type_by_token(&Token::new("int64[]"));

            prim.create_attribute(
                usd_geom_tokens().ids.as_str(),
                &int64_array_type,
                false,
                Some(Variability::Varying),
            )
            .unwrap_or_else(Attribute::invalid)
        };

        apply_optional_default(attr, default_value)
    }

    // ========================================================================
    // Compute Extent
    // ========================================================================

    /// Compute the extent for the point cloud defined by points and widths.
    ///
    /// Returns true upon success, false if widths and points are different
    /// sized arrays.
    ///
    /// On success, extent will contain the axis-aligned bounding box of the
    /// point cloud defined by points with the given widths.
    ///
    /// Matches C++ `ComputeExtent(const VtVec3fArray& points, const VtFloatArray& widths, VtVec3fArray* extent)`.
    pub fn compute_extent(points: &[Vec3f], widths: &[f32], extent: &mut [Vec3f; 2]) -> bool {
        if points.is_empty() {
            return false;
        }

        // If widths array is provided, it must match points size
        if !widths.is_empty() && widths.len() != points.len() {
            return false;
        }

        let mut min = Vec3f::new(f32::MAX, f32::MAX, f32::MAX);
        let mut max = Vec3f::new(f32::MIN, f32::MIN, f32::MIN);

        for (i, point) in points.iter().enumerate() {
            // Calculate radius from width (width is diameter)
            let radius = if !widths.is_empty() {
                widths[i] * 0.5
            } else {
                0.0
            };

            // Expand bounds by radius in all directions
            min.x = min.x.min(point.x - radius);
            min.y = min.y.min(point.y - radius);
            min.z = min.z.min(point.z - radius);
            max.x = max.x.max(point.x + radius);
            max.y = max.y.max(point.y + radius);
            max.z = max.z.max(point.z + radius);
        }

        extent[0] = min;
        extent[1] = max;
        true
    }

    /// Compute the extent as if the matrix transform was first applied.
    ///
    /// Matches C++ `ComputeExtent(const VtVec3fArray& points, const VtFloatArray& widths, const GfMatrix4d& transform, VtVec3fArray* extent)`.
    pub fn compute_extent_with_transform(
        points: &[Vec3f],
        widths: &[f32],
        transform: &Matrix4d,
        extent: &mut [Vec3f; 2],
    ) -> bool {
        if points.is_empty() {
            return false;
        }

        // If widths array is provided, it must match points size
        if !widths.is_empty() && widths.len() != points.len() {
            return false;
        }

        // Per-axis row lengths of the upper 3x3 (matching C++ axisLengths)
        let row0_len = ((transform[0][0] * transform[0][0]
            + transform[0][1] * transform[0][1]
            + transform[0][2] * transform[0][2])
            .sqrt()) as f32;
        let row1_len = ((transform[1][0] * transform[1][0]
            + transform[1][1] * transform[1][1]
            + transform[1][2] * transform[1][2])
            .sqrt()) as f32;
        let row2_len = ((transform[2][0] * transform[2][0]
            + transform[2][1] * transform[2][1]
            + transform[2][2] * transform[2][2])
            .sqrt()) as f32;

        let mut min = Vec3f::new(f32::MAX, f32::MAX, f32::MAX);
        let mut max = Vec3f::new(f32::MIN, f32::MIN, f32::MIN);

        for (i, point) in points.iter().enumerate() {
            let point_d = usd_gf::vec3::Vec3d::new(point.x as f64, point.y as f64, point.z as f64);
            let transformed_d = transform.transform_point(&point_d);
            let transformed = Vec3f::new(
                transformed_d.x as f32,
                transformed_d.y as f32,
                transformed_d.z as f32,
            );

            let half_width = if !widths.is_empty() {
                widths[i] * 0.5
            } else {
                0.0
            };

            // Per-axis expansion matching C++ axisLengths approach
            min.x = min.x.min(transformed.x - half_width * row0_len);
            min.y = min.y.min(transformed.y - half_width * row1_len);
            min.z = min.z.min(transformed.z - half_width * row2_len);
            max.x = max.x.max(transformed.x + half_width * row0_len);
            max.y = max.y.max(transformed.y + half_width * row1_len);
            max.z = max.z.max(transformed.z + half_width * row2_len);
        }

        extent[0] = min;
        extent[1] = max;
        true
    }

    // ========================================================================
    // Get Methods (Value Retrieval)
    // ========================================================================

    /// Get the widths at the specified time.
    ///
    /// Matches C++ `GetWidths(VtFloatArray* widths, UsdTimeCode time)`.
    pub fn get_widths(&self, time: TimeCode) -> Option<usd_vt::Array<f32>> {
        self.get_widths_attr().get_typed::<usd_vt::Array<f32>>(time)
    }

    /// Get the ids at the specified time.
    ///
    /// Matches C++ `GetIds(VtInt64Array* ids, UsdTimeCode time)`.
    pub fn get_ids(&self, time: TimeCode) -> Option<usd_vt::Array<i64>> {
        self.get_ids_attr().get_typed::<usd_vt::Array<i64>>(time)
    }

    // ========================================================================
    // Compute Extent At Time
    // ========================================================================

    /// Compute the extent for the points at the specified time.
    ///
    /// Returns true on success, false if extent was unable to be calculated.
    /// On success, extent will contain the axis-aligned bounding box of the points.
    ///
    /// Matches C++ `ComputeExtentAtTime(VtVec3fArray* extent, UsdTimeCode time, UsdTimeCode baseTime)`.
    pub fn compute_extent_at_time(
        &self,
        extent: &mut Vec<Vec3f>,
        time: TimeCode,
        base_time: TimeCode,
    ) -> bool {
        // Get points at the specified time
        let mut points = Vec::new();
        if !self
            .inner
            .compute_points_at_time(&mut points, time, base_time)
        {
            return false;
        }

        if points.is_empty() {
            return false;
        }

        // Get widths if available
        let widths_attr = self.get_widths_attr();
        let widths: Vec<f32> = if widths_attr.is_valid() {
            widths_attr
                .get(time)
                .map(|widths_value| widths_vec_from_usd_value(&widths_value))
                .unwrap_or_default()
        } else {
            Vec::new()
        };

        // If widths are empty or don't match points size, use PointBased::compute_extent
        if widths.is_empty() || widths.len() != points.len() {
            let mut extent_array = [Vec3f::new(0.0, 0.0, 0.0), Vec3f::new(0.0, 0.0, 0.0)];
            if !PointBased::compute_extent(&points, &mut extent_array) {
                return false;
            }
            extent.clear();
            extent.push(extent_array[0]);
            extent.push(extent_array[1]);
            return true;
        }

        // Compute extent with widths
        let mut extent_array = [Vec3f::new(0.0, 0.0, 0.0), Vec3f::new(0.0, 0.0, 0.0)];
        if !Self::compute_extent(&points, &widths, &mut extent_array) {
            return false;
        }

        extent.clear();
        extent.push(extent_array[0]);
        extent.push(extent_array[1]);
        true
    }

    /// Compute the extent for the points at the specified time with transform.
    ///
    /// Returns true on success, false if extent was unable to be calculated.
    ///
    /// Matches C++ `ComputeExtentAtTime(VtVec3fArray* extent, UsdTimeCode time, UsdTimeCode baseTime, const GfMatrix4d& transform)`.
    pub fn compute_extent_at_time_with_transform(
        &self,
        extent: &mut Vec<Vec3f>,
        time: TimeCode,
        base_time: TimeCode,
        transform: &usd_gf::matrix4::Matrix4d,
    ) -> bool {
        // Get points at the specified time
        let mut points = Vec::new();
        if !self
            .inner
            .compute_points_at_time(&mut points, time, base_time)
        {
            return false;
        }

        if points.is_empty() {
            return false;
        }

        // Get widths if available
        let widths_attr = self.get_widths_attr();
        let widths: Vec<f32> = if widths_attr.is_valid() {
            widths_attr
                .get(time)
                .map(|widths_value| widths_vec_from_usd_value(&widths_value))
                .unwrap_or_default()
        } else {
            Vec::new()
        };

        // If widths are empty or don't match points size, use PointBased::compute_extent_with_transform
        if widths.is_empty() || widths.len() != points.len() {
            let mut extent_array = [Vec3f::new(0.0, 0.0, 0.0), Vec3f::new(0.0, 0.0, 0.0)];
            if !PointBased::compute_extent_with_transform(&points, transform, &mut extent_array) {
                return false;
            }
            extent.clear();
            extent.push(extent_array[0]);
            extent.push(extent_array[1]);
            return true;
        }

        // Compute extent with widths and transform
        let mut extent_array = [Vec3f::new(0.0, 0.0, 0.0), Vec3f::new(0.0, 0.0, 0.0)];
        if !Self::compute_extent_with_transform(&points, &widths, transform, &mut extent_array) {
            return false;
        }

        extent.clear();
        extent.push(extent_array[0]);
        extent.push(extent_array[1]);
        true
    }

    /// Compute the extent for the points at multiple times.
    ///
    /// Returns true on success, false if extent was unable to be calculated.
    ///
    /// Matches C++ `ComputeExtentAtTimes(std::vector<VtVec3fArray>* extents, const std::vector<UsdTimeCode>& times, UsdTimeCode baseTime)`.
    pub fn compute_extent_at_times(
        &self,
        extents: &mut Vec<Vec<Vec3f>>,
        times: &[TimeCode],
        base_time: TimeCode,
    ) -> bool {
        let num_samples = times.len();
        extents.clear();
        extents.reserve(num_samples);

        for &time in times {
            let mut extent = Vec::new();
            if !self.compute_extent_at_time(&mut extent, time, base_time) {
                return false;
            }
            extents.push(extent);
        }

        true
    }

    /// Compute the extent for the points at multiple times with transform.
    ///
    /// Returns true on success, false if extent was unable to be calculated.
    ///
    /// Matches C++ `ComputeExtentAtTimes(std::vector<VtVec3fArray>* extents, const std::vector<UsdTimeCode>& times, UsdTimeCode baseTime, const GfMatrix4d& transform)`.
    pub fn compute_extent_at_times_with_transform(
        &self,
        extents: &mut Vec<Vec<Vec3f>>,
        times: &[TimeCode],
        base_time: TimeCode,
        transform: &usd_gf::matrix4::Matrix4d,
    ) -> bool {
        let num_samples = times.len();
        extents.clear();
        extents.reserve(num_samples);

        for &time in times {
            let mut extent = Vec::new();
            if !self.compute_extent_at_time_with_transform(&mut extent, time, base_time, transform)
            {
                return false;
            }
            extents.push(extent);
        }

        true
    }

    /// Returns the number of points as defined by the size of the
    /// points array at time_code.
    ///
    /// Matches C++ `GetPointCount()`.
    pub fn get_point_count(&self, time_code: TimeCode) -> usize {
        let points_attr = self.inner.get_points_attr();
        if !points_attr.is_valid() {
            return 0;
        }

        if let Some(value) = points_attr.get(time_code) {
            if let Some(points) = value.get::<Vec<Vec3f>>() {
                return points.len();
            } else if let Some(points) = value.get::<usd_vt::Array<Vec3f>>() {
                return points.len();
            }
        }
        0
    }

    /// Return a vector of names of all pre-declared attributes for this schema
    /// class and all its ancestor classes.
    ///
    /// Matches C++ `GetSchemaAttributeNames(bool includeInherited)`.
    pub fn get_schema_attribute_names(include_inherited: bool) -> Vec<Token> {
        let local_names = vec![
            usd_geom_tokens().widths.clone(),
            usd_geom_tokens().ids.clone(),
        ];

        if include_inherited {
            let mut all_names = PointBased::get_schema_attribute_names(true);
            all_names.extend(local_names);
            all_names
        } else {
            local_names
        }
    }
}

impl PartialEq for Points {
    fn eq(&self, other: &Self) -> bool {
        self.inner == other.inner
    }
}

impl Eq for Points {}
