//! UsdGeomPlane - plane geometry schema.
//!
//! Port of pxr/usd/usdGeom/plane.h/cpp
//!
//! Defines a primitive plane, centered at the origin.

use super::gprim::Gprim;
use super::tokens::usd_geom_tokens;
use usd_core::attribute::Variability;
use usd_core::{Attribute, Prim, Stage};
use usd_gf::bbox3d::BBox3d;
use usd_gf::matrix4::Matrix4d;
use usd_gf::range::Range3d;
use usd_gf::vec3::Vec3d;
use usd_gf::vec3::Vec3f;
use usd_sdf::ValueTypeRegistry;
use usd_tf::Token;
use usd_vt::Value;

// ============================================================================
// Plane
// ============================================================================

/// Plane geometry schema.
///
/// Defines a primitive plane, centered at the origin, and is defined by
/// a cardinal axis, width, and length. The plane is double-sided by default.
///
/// Matches C++ `UsdGeomPlane`.
#[derive(Debug, Clone)]
pub struct Plane {
    /// Base gprim schema.
    inner: Gprim,
}

impl Plane {
    /// Creates a Plane schema from a prim.
    ///
    /// Matches C++ `UsdGeomPlane(const UsdPrim& prim)`.
    pub fn new(prim: Prim) -> Self {
        Self {
            inner: Gprim::new(prim),
        }
    }

    /// Creates a Plane schema from a Gprim schema.
    ///
    /// Matches C++ `UsdGeomPlane(const UsdSchemaBase& schemaObj)`.
    pub fn from_gprim(gprim: Gprim) -> Self {
        Self { inner: gprim }
    }

    /// Creates an invalid Plane schema.
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
        Token::new("Plane")
    }

    /// Return a Plane holding the prim adhering to this schema at path on stage.
    ///
    /// Matches C++ `UsdGeomPlane::Get(const UsdStagePtr &stage, const SdfPath &path)`.
    pub fn get(stage: &Stage, path: &usd_sdf::Path) -> Self {
        if let Some(prim) = stage.get_prim_at_path(path) {
            Self::new(prim)
        } else {
            Self::invalid()
        }
    }

    /// Attempt to ensure a prim adhering to this schema at path is defined on this stage.
    ///
    /// Matches C++ `UsdGeomPlane::Define(const UsdStagePtr &stage, const SdfPath &path)`.
    pub fn define(stage: &Stage, path: &usd_sdf::Path) -> Self {
        match stage.define_prim(path.get_string(), Self::schema_type_name().as_str()) {
            Ok(prim) => Self::new(prim),
            Err(_) => Self::invalid(),
        }
    }

    // ========================================================================
    // DoubleSided
    // ========================================================================

    /// Returns the doubleSided attribute.
    ///
    /// Planes are double-sided by default.
    ///
    /// Matches C++ `GetDoubleSidedAttr()`.
    pub fn get_double_sided_attr(&self) -> Attribute {
        self.inner.get_double_sided_attr()
    }

    /// Creates the doubleSided attribute.
    ///
    /// Matches C++ `CreateDoubleSidedAttr()`.
    pub fn create_double_sided_attr(
        &self,
        _default_value: Option<Value>,
        _write_sparsely: bool,
    ) -> Attribute {
        self.inner.create_double_sided_attr()
    }

    // ========================================================================
    // Width
    // ========================================================================

    /// Returns the width attribute.
    ///
    /// The width of the plane.
    ///
    /// Matches C++ `GetWidthAttr()`.
    pub fn get_width_attr(&self) -> Attribute {
        let prim = self.inner.prim();
        prim.get_attribute(usd_geom_tokens().width.as_str())
            .unwrap_or_else(Attribute::invalid)
    }

    /// Creates the width attribute.
    ///
    /// Matches C++ `CreateWidthAttr()`.
    pub fn create_width_attr(
        &self,
        _default_value: Option<Value>,
        _write_sparsely: bool,
    ) -> Attribute {
        let prim = self.inner.prim();
        if !prim.is_valid() {
            return Attribute::invalid();
        }

        if prim.has_authored_attribute(usd_geom_tokens().width.as_str()) {
            return prim
                .get_attribute(usd_geom_tokens().width.as_str())
                .unwrap_or_else(|| Attribute::invalid());
        }

        let registry = ValueTypeRegistry::instance();
        let double_type = registry.find_type_by_token(&Token::new("double"));

        prim.create_attribute(
            usd_geom_tokens().width.as_str(),
            &double_type,
            false,                      // not custom
            Some(Variability::Varying), // can vary over time
        )
        .unwrap_or_else(Attribute::invalid)
    }

    // ========================================================================
    // Length
    // ========================================================================

    /// Returns the length attribute.
    ///
    /// The length of the plane.
    ///
    /// Matches C++ `GetLengthAttr()`.
    pub fn get_length_attr(&self) -> Attribute {
        let prim = self.inner.prim();
        prim.get_attribute(usd_geom_tokens().length.as_str())
            .unwrap_or_else(Attribute::invalid)
    }

    /// Creates the length attribute.
    ///
    /// Matches C++ `CreateLengthAttr()`.
    pub fn create_length_attr(
        &self,
        _default_value: Option<Value>,
        _write_sparsely: bool,
    ) -> Attribute {
        let prim = self.inner.prim();
        if !prim.is_valid() {
            return Attribute::invalid();
        }

        if prim.has_authored_attribute(usd_geom_tokens().length.as_str()) {
            return prim
                .get_attribute(usd_geom_tokens().length.as_str())
                .unwrap_or_else(|| Attribute::invalid());
        }

        let registry = ValueTypeRegistry::instance();
        let double_type = registry.find_type_by_token(&Token::new("double"));

        prim.create_attribute(
            usd_geom_tokens().length.as_str(),
            &double_type,
            false,                      // not custom
            Some(Variability::Varying), // can vary over time
        )
        .unwrap_or_else(Attribute::invalid)
    }

    // ========================================================================
    // Axis
    // ========================================================================

    /// Returns the axis attribute.
    ///
    /// The axis along which the surface of the plane is aligned.
    ///
    /// Matches C++ `GetAxisAttr()`.
    pub fn get_axis_attr(&self) -> Attribute {
        let prim = self.inner.prim();
        prim.get_attribute(usd_geom_tokens().axis.as_str())
            .unwrap_or_else(Attribute::invalid)
    }

    /// Creates the axis attribute.
    ///
    /// Matches C++ `CreateAxisAttr()`.
    pub fn create_axis_attr(
        &self,
        _default_value: Option<Value>,
        _write_sparsely: bool,
    ) -> Attribute {
        let prim = self.inner.prim();
        if !prim.is_valid() {
            return Attribute::invalid();
        }

        if prim.has_authored_attribute(usd_geom_tokens().axis.as_str()) {
            return prim
                .get_attribute(usd_geom_tokens().axis.as_str())
                .unwrap_or_else(|| Attribute::invalid());
        }

        let registry = ValueTypeRegistry::instance();
        let token_type = registry.find_type_by_token(&Token::new("token"));

        prim.create_attribute(
            usd_geom_tokens().axis.as_str(),
            &token_type,
            false,                      // not custom
            Some(Variability::Uniform), // uniform
        )
        .unwrap_or_else(Attribute::invalid)
    }

    // ========================================================================
    // Extent
    // ========================================================================

    /// Returns the extent attribute.
    ///
    /// Extent is re-defined on Plane only to provide a fallback value.
    ///
    /// Matches C++ `GetExtentAttr()`.
    pub fn get_extent_attr(&self) -> Attribute {
        self.inner.boundable().get_extent_attr()
    }

    /// Creates the extent attribute.
    ///
    /// Matches C++ `CreateExtentAttr()`.
    pub fn create_extent_attr(
        &self,
        _default_value: Option<Value>,
        _write_sparsely: bool,
    ) -> Attribute {
        self.inner.boundable().create_extent_attr()
    }

    // ========================================================================
    // Compute Extent
    // ========================================================================

    /// Compute the extent for the plane defined by width, length, and axis.
    ///
    /// Returns true upon success, false if unable to calculate extent.
    ///
    /// Matches C++ `ComputeExtent(double width, double length, const TfToken& axis, VtVec3fArray* extent)`.
    pub fn compute_extent(width: f64, length: f64, axis: &Token, extent: &mut [Vec3f; 2]) -> bool {
        let max = match Self::compute_extent_max(width, length, axis) {
            Some(m) => m,
            None => return false,
        };
        extent[0] = Vec3f::new(-max.x, -max.y, -max.z);
        extent[1] = max;
        true
    }

    /// Compute the extent as if the matrix transform was first applied.
    ///
    /// Matches C++ `ComputeExtent(double width, double length, const TfToken& axis, const GfMatrix4d& transform, VtVec3fArray* extent)`.
    pub fn compute_extent_with_transform(
        width: f64,
        length: f64,
        axis: &Token,
        transform: &Matrix4d,
        extent: &mut [Vec3f; 2],
    ) -> bool {
        let max = match Self::compute_extent_max(width, length, axis) {
            Some(m) => m,
            None => return false,
        };

        // Create bounding box from plane range
        let range = Range3d::new(
            Vec3d::new(-max.x as f64, -max.y as f64, -max.z as f64),
            Vec3d::new(max.x as f64, max.y as f64, max.z as f64),
        );
        let bbox = BBox3d::from_range_matrix(range, *transform);
        let aligned_range = bbox.compute_aligned_range();

        let min_vec = aligned_range.min();
        let max_vec = aligned_range.max();
        extent[0] = Vec3f::new(min_vec.x as f32, min_vec.y as f32, min_vec.z as f32);
        extent[1] = Vec3f::new(max_vec.x as f32, max_vec.y as f32, max_vec.z as f32);
        true
    }

    /// Helper function to compute extent max based on axis.
    fn compute_extent_max(width: f64, length: f64, axis: &Token) -> Option<Vec3f> {
        let half_width = (width * 0.5) as f32;
        let half_length = (length * 0.5) as f32;

        let axis_str = axis.as_str();
        if axis_str == usd_geom_tokens().x.as_str() {
            Some(Vec3f::new(0.0, half_length, half_width))
        } else if axis_str == usd_geom_tokens().y.as_str() {
            Some(Vec3f::new(half_width, 0.0, half_length))
        } else if axis_str == usd_geom_tokens().z.as_str() {
            Some(Vec3f::new(half_width, half_length, 0.0))
        } else {
            None // invalid axis
        }
    }

    // ========================================================================
    // Get Methods (Value Retrieval)
    // ========================================================================

    /// Get the width at the specified time.
    ///
    /// Matches C++ `GetWidth(double* width, UsdTimeCode time)`.
    pub fn get_width(&self, time: usd_sdf::TimeCode) -> Option<f64> {
        self.get_width_attr().get_typed::<f64>(time)
    }

    /// Get the length at the specified time.
    ///
    /// Matches C++ `GetLength(double* length, UsdTimeCode time)`.
    pub fn get_length(&self, time: usd_sdf::TimeCode) -> Option<f64> {
        self.get_length_attr().get_typed::<f64>(time)
    }

    /// Get the axis at the specified time.
    ///
    /// Matches C++ `GetAxis(TfToken* axis, UsdTimeCode time)`.
    pub fn get_axis(&self, time: usd_sdf::TimeCode) -> Option<Token> {
        self.get_axis_attr().get_typed::<Token>(time)
    }

    // ========================================================================
    // Compute Extent At Time
    // ========================================================================

    /// Compute the extent for the plane at the specified time.
    ///
    /// Returns true on success, false if extent was unable to be calculated.
    /// On success, extent will contain the axis-aligned bounding box of the plane.
    ///
    /// Matches C++ `ComputeExtentAtTime(VtVec3fArray* extent, UsdTimeCode time, UsdTimeCode baseTime)`.
    pub fn compute_extent_at_time(
        &self,
        extent: &mut Vec<Vec3f>,
        time: usd_sdf::TimeCode,
        _base_time: usd_sdf::TimeCode,
    ) -> bool {
        // Get width, length, and axis at the specified time
        let width = self.get_width(time).unwrap_or(2.0);
        let length = self.get_length(time).unwrap_or(2.0);
        let axis = match self.get_axis(time) {
            Some(a) => a,
            None => usd_geom_tokens().z.clone(), // default axis
        };

        // Compute extent
        let mut extent_array = [Vec3f::new(0.0, 0.0, 0.0), Vec3f::new(0.0, 0.0, 0.0)];
        if !Self::compute_extent(width, length, &axis, &mut extent_array) {
            return false;
        }

        extent.clear();
        extent.push(extent_array[0]);
        extent.push(extent_array[1]);
        true
    }

    /// Compute the extent for the plane at the specified time with transform.
    ///
    /// Returns true on success, false if extent was unable to be calculated.
    ///
    /// Matches C++ `ComputeExtentAtTime(VtVec3fArray* extent, UsdTimeCode time, UsdTimeCode baseTime, const GfMatrix4d& transform)`.
    pub fn compute_extent_at_time_with_transform(
        &self,
        extent: &mut Vec<Vec3f>,
        time: usd_sdf::TimeCode,
        _base_time: usd_sdf::TimeCode,
        transform: &Matrix4d,
    ) -> bool {
        // Get width, length, and axis at the specified time
        let width = self.get_width(time).unwrap_or(2.0);
        let length = self.get_length(time).unwrap_or(2.0);
        let axis = match self.get_axis(time) {
            Some(a) => a,
            None => usd_geom_tokens().z.clone(), // default axis
        };

        // Compute extent with transform
        let mut extent_array = [Vec3f::new(0.0, 0.0, 0.0), Vec3f::new(0.0, 0.0, 0.0)];
        if !Self::compute_extent_with_transform(width, length, &axis, transform, &mut extent_array)
        {
            return false;
        }

        extent.clear();
        extent.push(extent_array[0]);
        extent.push(extent_array[1]);
        true
    }

    /// Compute the extent for the plane at multiple times.
    ///
    /// Returns true on success, false if extent was unable to be calculated.
    ///
    /// Matches C++ `ComputeExtentAtTimes(std::vector<VtVec3fArray>* extents, const std::vector<UsdTimeCode>& times, UsdTimeCode baseTime)`.
    pub fn compute_extent_at_times(
        &self,
        extents: &mut Vec<Vec<Vec3f>>,
        times: &[usd_sdf::TimeCode],
        base_time: usd_sdf::TimeCode,
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

    /// Compute the extent for the plane at multiple times with transform.
    ///
    /// Returns true on success, false if extent was unable to be calculated.
    ///
    /// Matches C++ `ComputeExtentAtTimes(std::vector<VtVec3fArray>* extents, const std::vector<UsdTimeCode>& times, UsdTimeCode baseTime, const GfMatrix4d& transform)`.
    pub fn compute_extent_at_times_with_transform(
        &self,
        extents: &mut Vec<Vec<Vec3f>>,
        times: &[usd_sdf::TimeCode],
        base_time: usd_sdf::TimeCode,
        transform: &Matrix4d,
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

    /// Return a vector of names of all pre-declared attributes for this schema
    /// class and all its ancestor classes.
    ///
    /// Matches C++ `GetSchemaAttributeNames(bool includeInherited)`.
    pub fn get_schema_attribute_names(include_inherited: bool) -> Vec<Token> {
        let local_names = vec![
            usd_geom_tokens().double_sided.clone(),
            usd_geom_tokens().width.clone(),
            usd_geom_tokens().length.clone(),
            usd_geom_tokens().axis.clone(),
            usd_geom_tokens().extent.clone(),
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

impl PartialEq for Plane {
    fn eq(&self, other: &Self) -> bool {
        self.inner == other.inner
    }
}

impl Eq for Plane {}
